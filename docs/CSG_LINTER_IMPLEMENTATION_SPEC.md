# CSG Linter Implementation Specification
## Context-Sensitive Grammar Validation with Vector & Semantic Metadata

**Version**: 1.0  
**Target**: Claude Code Agent Execution  
**Project**: ob-poc KYC/UBO Onboarding Platform  
**Date**: 2025-11-29

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Architecture Overview](#2-architecture-overview)
3. [Database Schema Changes](#3-database-schema-changes)
4. [Rust Module Implementation](#4-rust-module-implementation)
5. [Seed Data & Migrations](#5-seed-data--migrations)
6. [Testing Strategy](#6-testing-strategy)
7. [Execution Checklist](#7-execution-checklist)

---

## 1. Executive Summary

### Problem
The current DSL validation pipeline checks syntax (NOM parser) and existence (RefTypeResolver queries DB), but cannot enforce **context-sensitive business rules**:

```clojure
;; Currently passes all validation, but is semantically wrong:
(entity.create-limited-company :name "Acme Corp" :as @company)
(document.catalog :document-type "PASSPORT" :entity-id @company)
;;                               ^^^^^^^^^ Passports are for people, not companies!
```

### Solution
Introduce a **CSG Linter** layer between parsing and semantic validation that:
1. Builds a symbol table with inferred entity types
2. Validates cross-statement references
3. Enforces applicability rules loaded from database metadata
4. Provides helpful error messages with valid alternatives

### Key Innovation: Vector-Enhanced Semantic Context
Beyond simple rule matching, we'll store:
- **Embedding vectors** for document types, attributes, and entity types
- **Semantic similarity scores** for fuzzy matching suggestions
- **Contextual metadata** enabling AI-assisted rule inference

---

## 2. Architecture Overview

### Current Pipeline
```
DSL Source → Parser (NOM) → AST → SemanticValidator → Executor
                  ↓              ↓
              Syntax OK      Refs exist in DB
```

### Target Pipeline
```
DSL Source → Parser (NOM) → AST → CSG Linter → SemanticValidator → Executor
                  ↓              ↓                    ↓
              Syntax OK     Context valid         Refs exist
                           (business rules)
                           (vector similarity)
```

### Module Structure
```
rust/src/dsl_v2/
├── mod.rs                    # Add: csg_linter, applicability_rules
├── parser.rs                 # Existing (no changes)
├── ast.rs                    # Existing (no changes)
├── validation.rs             # UPDATE: Add CSG error codes
├── semantic_validator.rs     # UPDATE: Integrate CSG linter
├── csg_linter.rs            # NEW: Main linter orchestration
├── applicability_rules.rs   # NEW: Rule loading & matching
└── semantic_context.rs      # NEW: Vector similarity operations
```

---

## 3. Database Schema Changes

### 3.1 Core Philosophy

We're adding three categories of metadata:

| Category | Purpose | Storage |
|----------|---------|---------|
| **Applicability Rules** | Hard constraints (passport → person only) | JSONB |
| **Semantic Context** | Descriptive metadata for AI/search | JSONB |
| **Vector Embeddings** | Similarity search, fuzzy matching | pgvector |

### 3.2 Migration: document_types Table

```sql
-- File: sql/migrations/001_csg_document_types_metadata.sql

BEGIN;

-- ============================================
-- DOCUMENT_TYPES: Add CSG Metadata Columns
-- ============================================

-- 1. Applicability Rules (hard constraints)
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS applicability JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".document_types.applicability IS 
'CSG applicability rules: entity_types[], jurisdictions[], client_types[], required_for[], excludes[]';

-- 2. Semantic Context (soft/descriptive metadata)
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS semantic_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".document_types.semantic_context IS 
'Rich semantic metadata: category, purpose, synonyms[], related_documents[], extraction_hints{}';

-- 3. Vector Embedding for similarity search
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS embedding vector(1536);

COMMENT ON COLUMN "ob-poc".document_types.embedding IS 
'OpenAI ada-002 or equivalent embedding of type description + semantic context';

-- 4. Embedding metadata
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_document_types_applicability 
ON "ob-poc".document_types USING GIN (applicability);

CREATE INDEX IF NOT EXISTS idx_document_types_semantic_context 
ON "ob-poc".document_types USING GIN (semantic_context);

CREATE INDEX IF NOT EXISTS idx_document_types_embedding 
ON "ob-poc".document_types USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 100);

COMMIT;
```

### 3.3 Migration: attribute_registry Table

```sql
-- File: sql/migrations/002_csg_attribute_registry_metadata.sql

BEGIN;

-- ============================================
-- ATTRIBUTE_REGISTRY: Add CSG Metadata Columns
-- ============================================

-- 1. Applicability Rules
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS applicability JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".attribute_registry.applicability IS 
'CSG applicability rules: entity_types[], required_for[], source_documents[], depends_on[]';

-- 2. Semantic Context
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS semantic_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".attribute_registry.semantic_context IS 
'Rich semantic metadata: category, synonyms[], extraction_patterns[], validation_hints{}';

-- 3. Vector Embedding
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS embedding vector(1536);

ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_attribute_registry_applicability 
ON "ob-poc".attribute_registry USING GIN (applicability);

CREATE INDEX IF NOT EXISTS idx_attribute_registry_semantic_context 
ON "ob-poc".attribute_registry USING GIN (semantic_context);

CREATE INDEX IF NOT EXISTS idx_attribute_registry_embedding 
ON "ob-poc".attribute_registry USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 100);

COMMIT;
```

### 3.4 Migration: entity_types Table

```sql
-- File: sql/migrations/003_csg_entity_types_metadata.sql

BEGIN;

-- ============================================
-- ENTITY_TYPES: Add CSG Metadata Columns
-- ============================================

-- 1. Semantic Context (entity types don't need applicability - they ARE the context)
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS semantic_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".entity_types.semantic_context IS 
'Rich semantic metadata: category, parent_type, synonyms[], typical_documents[], typical_attributes[]';

-- 2. Type Hierarchy (for wildcard matching)
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS parent_type_id UUID REFERENCES "ob-poc".entity_types(entity_type_id);

ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS type_hierarchy_path TEXT[];

COMMENT ON COLUMN "ob-poc".entity_types.type_hierarchy_path IS 
'Materialized path for efficient ancestor queries, e.g., ["ENTITY", "LEGAL_ENTITY", "LIMITED_COMPANY"]';

-- 3. Vector Embedding
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS embedding vector(1536);

ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_entity_types_semantic_context 
ON "ob-poc".entity_types USING GIN (semantic_context);

CREATE INDEX IF NOT EXISTS idx_entity_types_parent 
ON "ob-poc".entity_types (parent_type_id);

CREATE INDEX IF NOT EXISTS idx_entity_types_hierarchy 
ON "ob-poc".entity_types USING GIN (type_hierarchy_path);

CREATE INDEX IF NOT EXISTS idx_entity_types_embedding 
ON "ob-poc".entity_types USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 50);

COMMIT;
```

### 3.5 Migration: cbus Table

```sql
-- File: sql/migrations/004_csg_cbus_metadata.sql

BEGIN;

-- ============================================
-- CBUS: Add CSG Metadata Columns
-- ============================================

-- 1. Client Classification (for applicability rules)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS client_type VARCHAR(50);

COMMENT ON COLUMN "ob-poc".cbus.client_type IS 
'Client classification: individual, corporate, fund, trust, partnership';

ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS jurisdiction VARCHAR(10);

COMMENT ON COLUMN "ob-poc".cbus.jurisdiction IS 
'Primary jurisdiction code (FK to master_jurisdictions)';

-- 2. Risk Context
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS risk_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".cbus.risk_context IS 
'Risk-related context: risk_rating, pep_exposure, sanctions_exposure, industry_codes[]';

-- 3. Onboarding Context (for state-aware validation)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS onboarding_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".cbus.onboarding_context IS 
'Onboarding state: stage, completed_steps[], pending_requirements[], override_rules[]';

-- 4. Semantic Context (for AI-assisted operations)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS semantic_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".cbus.semantic_context IS 
'Rich semantic metadata: business_description, industry_keywords[], related_entities[]';

-- 5. Vector Embedding (for similarity search across CBUs)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS embedding vector(1536);

ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cbus_client_type ON "ob-poc".cbus(client_type);
CREATE INDEX IF NOT EXISTS idx_cbus_jurisdiction ON "ob-poc".cbus(jurisdiction);
CREATE INDEX IF NOT EXISTS idx_cbus_risk_context ON "ob-poc".cbus USING GIN (risk_context);
CREATE INDEX IF NOT EXISTS idx_cbus_onboarding_context ON "ob-poc".cbus USING GIN (onboarding_context);
CREATE INDEX IF NOT EXISTS idx_cbus_semantic_context ON "ob-poc".cbus USING GIN (semantic_context);

CREATE INDEX IF NOT EXISTS idx_cbus_embedding 
ON "ob-poc".cbus USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 100);

-- Add FK constraint for jurisdiction
ALTER TABLE "ob-poc".cbus
ADD CONSTRAINT fk_cbus_jurisdiction 
FOREIGN KEY (jurisdiction) REFERENCES "ob-poc".master_jurisdictions(jurisdiction_code)
ON DELETE SET NULL;

COMMIT;
```

### 3.6 New Table: csg_validation_rules

```sql
-- File: sql/migrations/005_csg_validation_rules_table.sql

BEGIN;

-- ============================================
-- CSG_VALIDATION_RULES: Centralized Rule Store
-- ============================================
-- This table allows rules to be managed independently of the entities they govern.
-- Rules can be versioned, A/B tested, and overridden per-CBU.

CREATE TABLE IF NOT EXISTS "ob-poc".csg_validation_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Rule identification
    rule_code VARCHAR(100) UNIQUE NOT NULL,
    rule_name VARCHAR(255) NOT NULL,
    rule_version INTEGER DEFAULT 1,
    
    -- What this rule applies to
    target_type VARCHAR(50) NOT NULL CHECK (target_type IN (
        'document_type',      -- Rule about document types
        'attribute',          -- Rule about attributes
        'entity_type',        -- Rule about entity types
        'verb',               -- Rule about DSL verbs
        'cross_reference'     -- Rule about relationships
    )),
    target_code VARCHAR(100),  -- Specific target (e.g., "PASSPORT") or NULL for all
    
    -- The rule definition
    rule_type VARCHAR(50) NOT NULL CHECK (rule_type IN (
        'entity_type_constraint',     -- Allowed entity types
        'jurisdiction_constraint',    -- Allowed jurisdictions
        'client_type_constraint',     -- Allowed client types
        'prerequisite',               -- Required prior operations
        'exclusion',                  -- Mutually exclusive items
        'co_occurrence',              -- Must appear together
        'sequence',                   -- Must appear in order
        'cardinality',                -- Min/max occurrences
        'custom'                      -- Custom validation function
    )),
    
    -- Rule parameters (the actual constraints)
    rule_params JSONB NOT NULL,
    
    -- Error handling
    error_code VARCHAR(10) NOT NULL,   -- e.g., "C001"
    error_message_template TEXT NOT NULL,
    suggestion_template TEXT,
    severity VARCHAR(20) DEFAULT 'error' CHECK (severity IN ('error', 'warning', 'info')),
    
    -- Metadata
    description TEXT,
    rationale TEXT,
    documentation_url TEXT,
    
    -- Lifecycle
    is_active BOOLEAN DEFAULT true,
    effective_from TIMESTAMPTZ DEFAULT NOW(),
    effective_until TIMESTAMPTZ,
    
    -- Audit
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_csg_rules_target ON "ob-poc".csg_validation_rules(target_type, target_code);
CREATE INDEX idx_csg_rules_type ON "ob-poc".csg_validation_rules(rule_type);
CREATE INDEX idx_csg_rules_active ON "ob-poc".csg_validation_rules(is_active) WHERE is_active = true;
CREATE INDEX idx_csg_rules_params ON "ob-poc".csg_validation_rules USING GIN (rule_params);

-- ============================================
-- CSG_RULE_OVERRIDES: Per-CBU Rule Overrides
-- ============================================
-- Allows specific CBUs to have custom rule behavior

CREATE TABLE IF NOT EXISTS "ob-poc".csg_rule_overrides (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID NOT NULL REFERENCES "ob-poc".csg_validation_rules(rule_id) ON DELETE CASCADE,
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    
    -- Override behavior
    override_type VARCHAR(50) NOT NULL CHECK (override_type IN (
        'disable',            -- Completely disable this rule for this CBU
        'downgrade',          -- Change error to warning
        'modify_params',      -- Use different parameters
        'add_exception'       -- Add specific exception values
    )),
    override_params JSONB,
    
    -- Approval workflow
    approved_by VARCHAR(255),
    approval_reason TEXT NOT NULL,
    approved_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    
    -- Audit
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(rule_id, cbu_id)
);

CREATE INDEX idx_csg_overrides_cbu ON "ob-poc".csg_rule_overrides(cbu_id);
CREATE INDEX idx_csg_overrides_rule ON "ob-poc".csg_rule_overrides(rule_id);

COMMIT;
```

### 3.7 New Table: csg_semantic_similarity_cache

```sql
-- File: sql/migrations/006_csg_similarity_cache.sql

BEGIN;

-- ============================================
-- CSG_SEMANTIC_SIMILARITY_CACHE
-- ============================================
-- Pre-computed similarity scores for fast suggestions

CREATE TABLE IF NOT EXISTS "ob-poc".csg_semantic_similarity_cache (
    cache_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Source item
    source_type VARCHAR(50) NOT NULL,  -- 'document_type', 'attribute', 'entity_type'
    source_code VARCHAR(100) NOT NULL,
    
    -- Target item
    target_type VARCHAR(50) NOT NULL,
    target_code VARCHAR(100) NOT NULL,
    
    -- Similarity metrics
    cosine_similarity FLOAT NOT NULL,
    levenshtein_distance INTEGER,
    semantic_relatedness FLOAT,  -- From knowledge graph if available
    
    -- Context
    relationship_type VARCHAR(50),  -- 'alternative', 'complement', 'parent', 'child'
    
    -- Cache management
    computed_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ DEFAULT NOW() + INTERVAL '7 days',
    
    UNIQUE(source_type, source_code, target_type, target_code)
);

CREATE INDEX idx_similarity_source ON "ob-poc".csg_semantic_similarity_cache(source_type, source_code);
CREATE INDEX idx_similarity_target ON "ob-poc".csg_semantic_similarity_cache(target_type, target_code);
CREATE INDEX idx_similarity_score ON "ob-poc".csg_semantic_similarity_cache(cosine_similarity DESC);
CREATE INDEX idx_similarity_expires ON "ob-poc".csg_semantic_similarity_cache(expires_at);

-- ============================================
-- FUNCTION: Refresh similarity cache for document types
-- ============================================

CREATE OR REPLACE FUNCTION "ob-poc".refresh_document_type_similarities()
RETURNS void AS $$
BEGIN
    -- Delete expired entries
    DELETE FROM "ob-poc".csg_semantic_similarity_cache
    WHERE expires_at < NOW();
    
    -- Insert new similarities based on embeddings
    INSERT INTO "ob-poc".csg_semantic_similarity_cache 
        (source_type, source_code, target_type, target_code, cosine_similarity, relationship_type, computed_at, expires_at)
    SELECT 
        'document_type', dt1.type_code,
        'document_type', dt2.type_code,
        1 - (dt1.embedding <=> dt2.embedding) as similarity,
        'alternative',
        NOW(),
        NOW() + INTERVAL '7 days'
    FROM "ob-poc".document_types dt1
    CROSS JOIN "ob-poc".document_types dt2
    WHERE dt1.type_code != dt2.type_code
      AND dt1.embedding IS NOT NULL
      AND dt2.embedding IS NOT NULL
      AND 1 - (dt1.embedding <=> dt2.embedding) > 0.5  -- Only store if reasonably similar
    ON CONFLICT (source_type, source_code, target_type, target_code) 
    DO UPDATE SET 
        cosine_similarity = EXCLUDED.cosine_similarity,
        computed_at = NOW(),
        expires_at = NOW() + INTERVAL '7 days';
END;
$$ LANGUAGE plpgsql;

COMMIT;
```

### 3.8 JSONB Schema Definitions

```sql
-- File: sql/migrations/007_csg_jsonb_schemas.sql
-- This file documents the expected JSONB structures (for reference, not executable)

/*
============================================
APPLICABILITY JSONB SCHEMA (document_types, attribute_registry)
============================================

{
    "entity_types": ["PROPER_PERSON_*", "BENEFICIAL_OWNER"],  -- Wildcards supported
    "jurisdictions": ["GB", "US", "EU"],                      -- ISO codes
    "client_types": ["individual", "corporate"],              -- Enum values
    "required_for": ["PROPER_PERSON_NATURAL"],               -- When this entity type, doc is required
    "excludes": ["DRIVERS_LICENSE_*"],                        -- Mutually exclusive
    "requires": ["PROOF_OF_ADDRESS"],                         -- Must have this first
    "min_count": 1,                                           -- Minimum required
    "max_count": null                                         -- Maximum allowed (null = unlimited)
}

============================================
SEMANTIC_CONTEXT JSONB SCHEMA (all tables)
============================================

{
    "category": "IDENTITY",                          -- High-level grouping
    "subcategory": "GOVERNMENT_ISSUED",              -- Finer grouping
    "purpose": "Verify identity of natural person",  -- Human-readable purpose
    "synonyms": ["ID", "identification", "passport document"],
    "related_items": ["NATIONAL_ID", "DRIVERS_LICENSE"],
    "extraction_hints": {
        "ocr_zones": ["mrz", "photo", "signature"],
        "expected_fields": ["full_name", "date_of_birth", "nationality"]
    },
    "validation_hints": {
        "expiry_check": true,
        "mrz_validation": true,
        "photo_match": true
    },
    "keywords": ["identity", "government", "photo", "travel"],
    "regulatory_references": ["FATF_R10", "4AMLD_Art13"]
}

============================================
RISK_CONTEXT JSONB SCHEMA (cbus)
============================================

{
    "risk_rating": "HIGH",                           -- LOW, MEDIUM, HIGH, PROHIBITED
    "risk_factors": [
        {"factor": "jurisdiction", "score": 0.8, "reason": "High-risk jurisdiction"},
        {"factor": "industry", "score": 0.6, "reason": "Cash-intensive business"}
    ],
    "pep_exposure": {
        "has_pep": true,
        "pep_entities": ["entity-uuid-1"],
        "pep_level": "DIRECT"
    },
    "sanctions_exposure": {
        "has_sanctions_hits": false,
        "screening_date": "2025-01-15"
    },
    "industry_codes": ["SIC_6411", "NACE_K65"]
}

============================================
ONBOARDING_CONTEXT JSONB SCHEMA (cbus)
============================================

{
    "stage": "documents_pending",                    -- Current stage
    "completed_steps": [
        "cbu_created",
        "entities_registered",
        "roles_assigned"
    ],
    "pending_requirements": [
        {"type": "document", "code": "PASSPORT", "entity_id": "..."},
        {"type": "attribute", "code": "date_of_birth", "entity_id": "..."}
    ],
    "override_rules": ["RULE_PASSPORT_REQUIRED"],   -- Disabled rules for this CBU
    "workflow_id": "uuid",
    "started_at": "2025-01-10T10:00:00Z",
    "expected_completion": "2025-01-20T10:00:00Z"
}

============================================
CSG_VALIDATION_RULES.rule_params JSONB SCHEMA
============================================

-- For rule_type = 'entity_type_constraint':
{
    "allowed_entity_types": ["PROPER_PERSON_*"],
    "wildcard_support": true
}

-- For rule_type = 'jurisdiction_constraint':
{
    "allowed_jurisdictions": ["GB", "US"],
    "denied_jurisdictions": ["IR", "KP"]
}

-- For rule_type = 'prerequisite':
{
    "required_operations": [
        {"domain": "cbu", "verb": "create"},
        {"domain": "entity", "verb": "create-*"}
    ]
}

-- For rule_type = 'co_occurrence':
{
    "must_have_all": ["CERT_OF_INCORPORATION", "ARTICLES_ASSOC"],
    "within_scope": "cbu"  -- Must all exist for same CBU
}

-- For rule_type = 'cardinality':
{
    "min": 1,
    "max": 3,
    "scope": "entity"  -- Per entity
}
*/
```

---

## 4. Rust Module Implementation

### 4.1 File: `rust/src/dsl_v2/csg_linter.rs`

```rust
//! Context-Sensitive Grammar Linter
//!
//! Validates DSL programs against business rules that depend on runtime context.
//! This is the core orchestration module for CSG validation.
//!
//! # Pipeline Position
//! ```text
//! Parser → AST → [CSG Linter] → SemanticValidator → Executor
//! ```
//!
//! # Three-Pass Architecture
//! 1. **Symbol Analysis**: Build symbol table, infer types
//! 2. **Reference Validation**: Check cross-statement references
//! 3. **Applicability Validation**: Enforce business rules from DB

use crate::dsl_v2::ast::{Argument, Program, Span, Statement, Value, VerbCall};
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticBuilder, DiagnosticCode, Severity, SourceSpan, ValidationContext,
};
use crate::dsl_v2::applicability_rules::{ApplicabilityRules, DocumentApplicability};
use crate::dsl_v2::semantic_context::SemanticContextStore;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// PUBLIC TYPES
// =============================================================================

/// Result of CSG linting
#[derive(Debug)]
pub struct LintResult {
    /// The original AST (passed through)
    pub ast: Program,
    /// Diagnostics generated during linting
    pub diagnostics: Vec<Diagnostic>,
    /// Context inferred from AST analysis
    pub inferred_context: InferredContext,
}

impl LintResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
    
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Warning)
    }
}

/// Context inferred from AST analysis
#[derive(Debug, Default)]
pub struct InferredContext {
    /// Symbol bindings: name → type info
    pub symbols: HashMap<String, SymbolInfo>,
    /// Operations that create CBUs
    pub cbu_creates: Vec<CbuCreate>,
    /// Operations that create entities
    pub entity_creates: Vec<EntityCreate>,
    /// Operations that reference entities
    pub entity_refs: Vec<EntityRef>,
    /// Operations that catalog documents
    pub document_catalogs: Vec<DocumentCatalog>,
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub domain: String,              // "cbu", "entity", "document"
    pub entity_type: Option<String>, // e.g., "LIMITED_COMPANY", "PROPER_PERSON"
    pub defined_at: SourceSpan,
    pub used: bool,                  // For unused symbol warnings
}

#[derive(Debug)]
pub struct CbuCreate {
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct EntityCreate {
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub entity_type: String,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct EntityRef {
    pub symbol: String,
    pub argument_key: String,        // Which argument referenced this
    pub expected_type: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct DocumentCatalog {
    pub symbol: Option<String>,
    pub document_type: String,
    pub cbu_ref: Option<String>,
    pub entity_ref: Option<String>,
    pub span: SourceSpan,
}

// =============================================================================
// CSG LINTER
// =============================================================================

pub struct CsgLinter {
    pool: PgPool,
    rules: ApplicabilityRules,
    semantic_store: SemanticContextStore,
    initialized: bool,
}

impl CsgLinter {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: pool.clone(),
            rules: ApplicabilityRules::default(),
            semantic_store: SemanticContextStore::new(pool),
            initialized: false,
        }
    }

    /// Initialize linter by loading rules from database
    pub async fn initialize(&mut self) -> Result<(), String> {
        self.rules = ApplicabilityRules::load(&self.pool).await?;
        self.semantic_store.initialize().await?;
        self.initialized = true;
        Ok(())
    }

    /// Main entry point: Lint a parsed AST
    pub async fn lint(
        &self,
        ast: Program,
        context: &ValidationContext,
        source: &str,
    ) -> LintResult {
        if !self.initialized {
            return LintResult {
                ast,
                diagnostics: vec![Diagnostic {
                    severity: Severity::Error,
                    span: SourceSpan::default(),
                    code: DiagnosticCode::InternalError,
                    message: "CSG Linter not initialized".to_string(),
                    suggestions: vec![],
                }],
                inferred_context: InferredContext::default(),
            };
        }

        let mut diagnostics = DiagnosticBuilder::new();
        let mut inferred = InferredContext::default();

        // Pass 1: Symbol analysis
        for statement in &ast.statements {
            if let Statement::VerbCall(vc) = statement {
                self.analyze_statement(vc, source, &mut inferred);
            }
        }

        // Pass 2: Reference validation
        self.validate_references(&inferred, source, &mut diagnostics);

        // Pass 3: Applicability validation
        self.validate_applicability(&ast, &inferred, context, source, &mut diagnostics).await;

        // Pass 4: Unused symbol warnings
        self.check_unused_symbols(&inferred, &mut diagnostics);

        LintResult {
            ast,
            diagnostics: diagnostics.build(),
            inferred_context: inferred,
        }
    }

    // =========================================================================
    // PASS 1: SYMBOL ANALYSIS
    // =========================================================================

    fn analyze_statement(
        &self,
        vc: &VerbCall,
        source: &str,
        inferred: &mut InferredContext,
    ) {
        // Extract symbol binding
        if let Some(ref binding) = vc.as_binding {
            let entity_type = self.infer_entity_type(vc);
            inferred.symbols.insert(
                binding.clone(),
                SymbolInfo {
                    name: binding.clone(),
                    domain: vc.domain.clone(),
                    entity_type,
                    defined_at: self.span_to_source_span(&vc.span, source),
                    used: false,
                },
            );
        }

        // Track specific operation types
        match (vc.domain.as_str(), vc.verb.as_str()) {
            ("cbu", "create") => {
                inferred.cbu_creates.push(CbuCreate {
                    symbol: vc.as_binding.clone(),
                    name: self.extract_string_arg(vc, "name"),
                    client_type: self.extract_string_arg(vc, "client-type"),
                    jurisdiction: self.extract_string_arg(vc, "jurisdiction"),
                    span: self.span_to_source_span(&vc.span, source),
                });
            }
            ("entity", verb) if verb.starts_with("create") => {
                let entity_type = self.infer_entity_type_from_verb_and_args(vc);
                inferred.entity_creates.push(EntityCreate {
                    symbol: vc.as_binding.clone(),
                    name: self.extract_string_arg(vc, "name"),
                    entity_type,
                    span: self.span_to_source_span(&vc.span, source),
                });
            }
            ("document", "catalog") => {
                if let Some(doc_type) = self.extract_string_arg(vc, "document-type") {
                    inferred.document_catalogs.push(DocumentCatalog {
                        symbol: vc.as_binding.clone(),
                        document_type: doc_type,
                        cbu_ref: self.extract_ref_arg(vc, "cbu-id"),
                        entity_ref: self.extract_ref_arg(vc, "entity-id"),
                        span: self.span_to_source_span(&vc.span, source),
                    });
                }
            }
            _ => {}
        }

        // Track all entity references
        for arg in &vc.arguments {
            if let Value::Reference(ref name) = arg.value {
                inferred.entity_refs.push(EntityRef {
                    symbol: name.clone(),
                    argument_key: arg.key.canonical(),
                    expected_type: self.expected_type_for_arg(&vc.domain, &vc.verb, &arg.key.canonical()),
                    span: self.span_to_source_span(&arg.value_span, source),
                });
            }
        }
    }

    // =========================================================================
    // PASS 2: REFERENCE VALIDATION
    // =========================================================================

    fn validate_references(
        &self,
        inferred: &InferredContext,
        _source: &str,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        for entity_ref in &inferred.entity_refs {
            // Mark symbol as used
            // (Note: we can't mutate inferred here, would need interior mutability)
            
            // Check symbol is defined
            match inferred.symbols.get(&entity_ref.symbol) {
                None => {
                    diagnostics.error(
                        DiagnosticCode::UndefinedSymbol,
                        entity_ref.span,
                        format!("undefined symbol '@{}'", entity_ref.symbol),
                    );
                }
                Some(symbol_info) => {
                    // Check type compatibility if we expect a specific type
                    if let (Some(ref expected), Some(ref actual)) = 
                        (&entity_ref.expected_type, &symbol_info.entity_type) 
                    {
                        if !self.types_compatible(expected, actual) {
                            diagnostics.error(
                                DiagnosticCode::SymbolTypeMismatch,
                                entity_ref.span,
                                format!(
                                    "type mismatch: '{}' expects {}, but '@{}' has type {}",
                                    entity_ref.argument_key,
                                    expected,
                                    entity_ref.symbol,
                                    actual
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    // =========================================================================
    // PASS 3: APPLICABILITY VALIDATION
    // =========================================================================

    async fn validate_applicability(
        &self,
        _ast: &Program,
        inferred: &InferredContext,
        context: &ValidationContext,
        _source: &str,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        // Validate each document catalog operation
        for doc_catalog in &inferred.document_catalogs {
            self.validate_document_applicability(doc_catalog, inferred, context, diagnostics).await;
        }
    }

    async fn validate_document_applicability(
        &self,
        doc_catalog: &DocumentCatalog,
        inferred: &InferredContext,
        context: &ValidationContext,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        let Some(rule) = self.rules.document_rules.get(&doc_catalog.document_type) else {
            // No rule = no constraint (or unknown doc type, but that's caught by SemanticValidator)
            return;
        };

        // Check entity type constraint
        if let Some(ref entity_sym) = doc_catalog.entity_ref {
            if let Some(symbol_info) = inferred.symbols.get(entity_sym) {
                if let Some(ref entity_type) = symbol_info.entity_type {
                    if !rule.applies_to_entity_type(entity_type) {
                        let suggestions = self.suggest_documents_for_entity(entity_type).await;
                        diagnostics.error(
                            DiagnosticCode::DocumentNotApplicableToEntityType,
                            doc_catalog.span,
                            format!(
                                "document type '{}' is not applicable to entity type '{}'",
                                doc_catalog.document_type, entity_type
                            ),
                        ).suggest(
                            "valid document types for this entity",
                            suggestions.join(", "),
                            0.8,
                        );
                    }
                }
            }
        }

        // Check jurisdiction constraint (using CBU's jurisdiction or context)
        let jurisdiction = doc_catalog.cbu_ref
            .as_ref()
            .and_then(|sym| inferred.symbols.get(sym))
            .and_then(|_| context.jurisdiction.clone())
            .or_else(|| context.jurisdiction.clone());

        if let Some(ref jurisdiction) = jurisdiction {
            if !rule.applies_to_jurisdiction(jurisdiction) {
                diagnostics.error(
                    DiagnosticCode::DocumentNotApplicableToJurisdiction,
                    doc_catalog.span,
                    format!(
                        "document type '{}' is not valid in jurisdiction '{}'",
                        doc_catalog.document_type, jurisdiction
                    ),
                );
            }
        }

        // Check client type constraint
        if let Some(ref client_type) = context.client_type {
            let client_type_str = format!("{:?}", client_type).to_lowercase();
            if !rule.applies_to_client_type(&client_type_str) {
                diagnostics.error(
                    DiagnosticCode::DocumentNotApplicableToClientType,
                    doc_catalog.span,
                    format!(
                        "document type '{}' is not valid for client type '{}'",
                        doc_catalog.document_type, client_type_str
                    ),
                );
            }
        }
    }

    // =========================================================================
    // PASS 4: UNUSED SYMBOL WARNINGS
    // =========================================================================

    fn check_unused_symbols(
        &self,
        inferred: &InferredContext,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        // Build set of used symbols
        let used_symbols: std::collections::HashSet<_> = inferred.entity_refs
            .iter()
            .map(|r| &r.symbol)
            .collect();

        for (name, info) in &inferred.symbols {
            if !used_symbols.contains(name) {
                diagnostics.warning(
                    DiagnosticCode::UnusedBinding,
                    info.defined_at,
                    format!("symbol '@{}' is defined but never used", name),
                );
            }
        }
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    fn infer_entity_type(&self, vc: &VerbCall) -> Option<String> {
        if vc.domain != "entity" {
            return None;
        }
        self.infer_entity_type_from_verb_and_args(vc).into()
    }

    fn infer_entity_type_from_verb_and_args(&self, vc: &VerbCall) -> String {
        // First check explicit :type argument
        if let Some(explicit_type) = self.extract_string_arg(vc, "type")
            .or_else(|| self.extract_string_arg(vc, "entity-type"))
        {
            return explicit_type.to_uppercase().replace('-', "_");
        }

        // Infer from verb name
        match vc.verb.as_str() {
            "create-limited-company" => "LIMITED_COMPANY".to_string(),
            "create-proper-person" | "create-natural-person" => "PROPER_PERSON".to_string(),
            "create-partnership" => "PARTNERSHIP".to_string(),
            "create-trust" => "TRUST".to_string(),
            "create" => "ENTITY".to_string(), // Generic
            _ => "UNKNOWN".to_string(),
        }
    }

    fn extract_string_arg(&self, vc: &VerbCall, key: &str) -> Option<String> {
        vc.arguments.iter()
            .find(|a| a.key.canonical() == key)
            .and_then(|a| a.value.as_string().map(|s| s.to_string()))
    }

    fn extract_ref_arg(&self, vc: &VerbCall, key: &str) -> Option<String> {
        vc.arguments.iter()
            .find(|a| a.key.canonical() == key)
            .and_then(|a| a.value.as_reference().map(|s| s.to_string()))
    }

    fn expected_type_for_arg(&self, domain: &str, verb: &str, arg_key: &str) -> Option<String> {
        // Map argument keys to expected entity types
        match arg_key {
            "person-id" => Some("PROPER_PERSON".to_string()),
            "company-id" => Some("LIMITED_COMPANY".to_string()),
            "partnership-id" => Some("PARTNERSHIP".to_string()),
            "trust-id" => Some("TRUST".to_string()),
            // entity-id is polymorphic - could be any entity
            "entity-id" if domain == "document" && verb == "catalog" => None,
            _ => None,
        }
    }

    fn types_compatible(&self, expected: &str, actual: &str) -> bool {
        // Direct match
        if expected == actual {
            return true;
        }
        
        // Wildcard match: "LIMITED_COMPANY_*" matches "LIMITED_COMPANY_PRIVATE"
        if expected.ends_with('*') {
            let prefix = &expected[..expected.len() - 1];
            return actual.starts_with(prefix);
        }
        
        // Hierarchy match: "PROPER_PERSON" matches "PROPER_PERSON_NATURAL"
        if actual.starts_with(expected) && actual.len() > expected.len() {
            let suffix = &actual[expected.len()..];
            return suffix.starts_with('_');
        }
        
        false
    }

    async fn suggest_documents_for_entity(&self, entity_type: &str) -> Vec<String> {
        // First try exact matches from rules
        let mut suggestions: Vec<String> = self.rules.document_rules.iter()
            .filter(|(_, rule)| rule.applies_to_entity_type(entity_type))
            .map(|(code, _)| code.clone())
            .collect();

        // If few results, supplement with semantic similarity
        if suggestions.len() < 3 {
            if let Ok(similar) = self.semantic_store
                .find_similar_documents(entity_type, 5)
                .await
            {
                for doc in similar {
                    if !suggestions.contains(&doc) {
                        suggestions.push(doc);
                    }
                }
            }
        }

        suggestions.truncate(5);
        suggestions
    }

    fn span_to_source_span(&self, span: &Span, source: &str) -> SourceSpan {
        // Calculate line and column from byte offset
        let mut line = 1u32;
        let mut last_newline = 0usize;

        for (i, ch) in source.char_indices() {
            if i >= span.start {
                break;
            }
            if ch == '\n' {
                line += 1;
                last_newline = i + 1;
            }
        }

        SourceSpan {
            line,
            column: (span.start - last_newline) as u32,
            offset: span.start as u32,
            length: (span.end - span.start) as u32,
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_types_compatible_exact() {
        let linter = CsgLinter::new(sqlx::PgPool::connect_lazy("").unwrap());
        assert!(linter.types_compatible("LIMITED_COMPANY", "LIMITED_COMPANY"));
        assert!(!linter.types_compatible("LIMITED_COMPANY", "PROPER_PERSON"));
    }

    #[test]
    fn test_types_compatible_wildcard() {
        let linter = CsgLinter::new(sqlx::PgPool::connect_lazy("").unwrap());
        assert!(linter.types_compatible("LIMITED_COMPANY_*", "LIMITED_COMPANY_PRIVATE"));
        assert!(linter.types_compatible("LIMITED_COMPANY_*", "LIMITED_COMPANY_PUBLIC"));
        assert!(!linter.types_compatible("LIMITED_COMPANY_*", "PROPER_PERSON"));
    }

    #[test]
    fn test_types_compatible_hierarchy() {
        let linter = CsgLinter::new(sqlx::PgPool::connect_lazy("").unwrap());
        assert!(linter.types_compatible("PROPER_PERSON", "PROPER_PERSON_NATURAL"));
        assert!(linter.types_compatible("PROPER_PERSON", "PROPER_PERSON_BENEFICIAL_OWNER"));
        assert!(!linter.types_compatible("PROPER_PERSON_NATURAL", "PROPER_PERSON"));
    }
}
```

### 4.2 File: `rust/src/dsl_v2/applicability_rules.rs`

```rust
//! Applicability Rules
//!
//! Loads and evaluates business rules from database metadata.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

// =============================================================================
// RULE STRUCTURES
// =============================================================================

/// All loaded applicability rules
#[derive(Debug, Default)]
pub struct ApplicabilityRules {
    pub document_rules: HashMap<String, DocumentApplicability>,
    pub attribute_rules: HashMap<String, AttributeApplicability>,
    pub entity_type_hierarchy: HashMap<String, Vec<String>>,
}

/// Applicability rules for a document type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentApplicability {
    #[serde(default)]
    pub entity_types: Vec<String>,
    
    #[serde(default)]
    pub jurisdictions: Vec<String>,
    
    #[serde(default)]
    pub client_types: Vec<String>,
    
    #[serde(default)]
    pub required_for: Vec<String>,
    
    #[serde(default)]
    pub excludes: Vec<String>,
    
    #[serde(default)]
    pub requires: Vec<String>,
    
    #[serde(default)]
    pub category: Option<String>,
}

impl DocumentApplicability {
    /// Check if document applies to given entity type (supports wildcards)
    pub fn applies_to_entity_type(&self, entity_type: &str) -> bool {
        if self.entity_types.is_empty() {
            return true; // No restriction
        }
        
        self.entity_types.iter().any(|allowed| {
            if allowed.ends_with('*') {
                let prefix = &allowed[..allowed.len() - 1];
                entity_type.starts_with(prefix)
            } else {
                allowed == entity_type || entity_type.starts_with(&format!("{}_", allowed))
            }
        })
    }

    /// Check if document applies to given jurisdiction
    pub fn applies_to_jurisdiction(&self, jurisdiction: &str) -> bool {
        if self.jurisdictions.is_empty() {
            return true;
        }
        self.jurisdictions.iter().any(|j| j == jurisdiction)
    }

    /// Check if document applies to given client type
    pub fn applies_to_client_type(&self, client_type: &str) -> bool {
        if self.client_types.is_empty() {
            return true;
        }
        self.client_types.iter().any(|c| c == client_type)
    }

    /// Check if document is required for given entity type
    pub fn is_required_for(&self, entity_type: &str) -> bool {
        self.required_for.iter().any(|req| {
            if req.ends_with('*') {
                let prefix = &req[..req.len() - 1];
                entity_type.starts_with(prefix)
            } else {
                req == entity_type
            }
        })
    }
}

/// Applicability rules for an attribute
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttributeApplicability {
    #[serde(default)]
    pub entity_types: Vec<String>,
    
    #[serde(default)]
    pub required_for: Vec<String>,
    
    #[serde(default)]
    pub source_documents: Vec<String>,
    
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl AttributeApplicability {
    pub fn applies_to_entity_type(&self, entity_type: &str) -> bool {
        if self.entity_types.is_empty() {
            return true;
        }
        self.entity_types.iter().any(|allowed| {
            if allowed.ends_with('*') {
                let prefix = &allowed[..allowed.len() - 1];
                entity_type.starts_with(prefix)
            } else {
                allowed == entity_type
            }
        })
    }
}

// =============================================================================
// RULE LOADING
// =============================================================================

impl ApplicabilityRules {
    /// Load all rules from database
    pub async fn load(pool: &PgPool) -> Result<Self, String> {
        let mut rules = Self::default();

        // Load document type rules
        rules.document_rules = Self::load_document_rules(pool).await?;
        
        // Load attribute rules
        rules.attribute_rules = Self::load_attribute_rules(pool).await?;
        
        // Load entity type hierarchy
        rules.entity_type_hierarchy = Self::load_entity_hierarchy(pool).await?;

        Ok(rules)
    }

    async fn load_document_rules(pool: &PgPool) -> Result<HashMap<String, DocumentApplicability>, String> {
        let rows = sqlx::query!(
            r#"SELECT type_code, applicability
               FROM "ob-poc".document_types
               WHERE is_active = true"#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load document rules: {}", e))?;

        let mut rules = HashMap::new();
        for row in rows {
            let applicability = row.applicability
                .and_then(|v| serde_json::from_value::<DocumentApplicability>(v).ok())
                .unwrap_or_default();
            rules.insert(row.type_code, applicability);
        }

        Ok(rules)
    }

    async fn load_attribute_rules(pool: &PgPool) -> Result<HashMap<String, AttributeApplicability>, String> {
        let rows = sqlx::query!(
            r#"SELECT semantic_id, applicability
               FROM "ob-poc".attribute_registry"#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load attribute rules: {}", e))?;

        let mut rules = HashMap::new();
        for row in rows {
            let applicability = row.applicability
                .and_then(|v| serde_json::from_value::<AttributeApplicability>(v).ok())
                .unwrap_or_default();
            rules.insert(row.semantic_id, applicability);
        }

        Ok(rules)
    }

    async fn load_entity_hierarchy(pool: &PgPool) -> Result<HashMap<String, Vec<String>>, String> {
        let rows = sqlx::query!(
            r#"SELECT type_code, type_hierarchy_path
               FROM "ob-poc".entity_types
               WHERE is_active = true"#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load entity hierarchy: {}", e))?;

        let mut hierarchy = HashMap::new();
        for row in rows {
            let path = row.type_hierarchy_path.unwrap_or_default();
            hierarchy.insert(row.type_code, path);
        }

        Ok(hierarchy)
    }

    /// Find valid documents for an entity type
    pub fn valid_documents_for_entity(&self, entity_type: &str) -> Vec<&str> {
        self.document_rules.iter()
            .filter(|(_, rule)| rule.applies_to_entity_type(entity_type))
            .map(|(code, _)| code.as_str())
            .collect()
    }

    /// Find required documents for an entity type
    pub fn required_documents_for_entity(&self, entity_type: &str) -> Vec<&str> {
        self.document_rules.iter()
            .filter(|(_, rule)| rule.is_required_for(entity_type))
            .map(|(code, _)| code.as_str())
            .collect()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_applicability_empty() {
        let rule = DocumentApplicability::default();
        assert!(rule.applies_to_entity_type("ANY_TYPE"));
        assert!(rule.applies_to_jurisdiction("ANY"));
    }

    #[test]
    fn test_document_applicability_exact_match() {
        let rule = DocumentApplicability {
            entity_types: vec!["PROPER_PERSON".to_string()],
            ..Default::default()
        };
        assert!(rule.applies_to_entity_type("PROPER_PERSON"));
        assert!(rule.applies_to_entity_type("PROPER_PERSON_NATURAL")); // Hierarchy
        assert!(!rule.applies_to_entity_type("LIMITED_COMPANY"));
    }

    #[test]
    fn test_document_applicability_wildcard() {
        let rule = DocumentApplicability {
            entity_types: vec!["LIMITED_COMPANY_*".to_string()],
            ..Default::default()
        };
        assert!(rule.applies_to_entity_type("LIMITED_COMPANY_PRIVATE"));
        assert!(rule.applies_to_entity_type("LIMITED_COMPANY_PUBLIC"));
        assert!(!rule.applies_to_entity_type("LIMITED_COMPANY")); // Exact doesn't match wildcard
        assert!(!rule.applies_to_entity_type("PROPER_PERSON"));
    }

    #[test]
    fn test_document_applicability_jurisdiction() {
        let rule = DocumentApplicability {
            jurisdictions: vec!["GB".to_string(), "US".to_string()],
            ..Default::default()
        };
        assert!(rule.applies_to_jurisdiction("GB"));
        assert!(rule.applies_to_jurisdiction("US"));
        assert!(!rule.applies_to_jurisdiction("DE"));
    }
}
```

### 4.3 File: `rust/src/dsl_v2/semantic_context.rs`

```rust
//! Semantic Context Store
//!
//! Provides vector-based semantic similarity for enhanced suggestions.

use sqlx::PgPool;
use std::collections::HashMap;

/// Store for semantic context and vector operations
pub struct SemanticContextStore {
    pool: PgPool,
    initialized: bool,
}

impl SemanticContextStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            initialized: false,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), String> {
        // Verify vector extension is available
        let _ = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM pg_extension WHERE extname = 'vector'"#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("pgvector not available: {}", e))?;

        self.initialized = true;
        Ok(())
    }

    /// Find semantically similar document types using vector embeddings
    pub async fn find_similar_documents(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        if !self.initialized {
            return Ok(vec![]);
        }

        // For now, return empty - full implementation requires embedding generation
        // In production, this would:
        // 1. Generate embedding for query
        // 2. Query document_types using cosine similarity
        // 3. Return top N results
        
        let results = sqlx::query_scalar!(
            r#"
            SELECT type_code
            FROM "ob-poc".document_types
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> (
                SELECT embedding FROM "ob-poc".entity_types 
                WHERE type_code = $1
                LIMIT 1
            )
            LIMIT $2
            "#,
            query,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(results)
    }

    /// Find semantically similar attributes
    pub async fn find_similar_attributes(
        &self,
        document_type: &str,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        if !self.initialized {
            return Ok(vec![]);
        }

        let results = sqlx::query_scalar!(
            r#"
            SELECT ar.semantic_id
            FROM "ob-poc".attribute_registry ar
            WHERE ar.embedding IS NOT NULL
            ORDER BY ar.embedding <=> (
                SELECT dt.embedding FROM "ob-poc".document_types dt
                WHERE dt.type_code = $1
                LIMIT 1
            )
            LIMIT $2
            "#,
            document_type,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(results)
    }

    /// Get pre-computed similarity scores from cache
    pub async fn get_cached_similarities(
        &self,
        source_type: &str,
        source_code: &str,
        target_type: &str,
        min_similarity: f64,
    ) -> Result<Vec<(String, f64)>, String> {
        let results = sqlx::query!(
            r#"
            SELECT target_code, cosine_similarity
            FROM "ob-poc".csg_semantic_similarity_cache
            WHERE source_type = $1
              AND source_code = $2
              AND target_type = $3
              AND cosine_similarity >= $4
              AND expires_at > NOW()
            ORDER BY cosine_similarity DESC
            "#,
            source_type,
            source_code,
            target_type,
            min_similarity
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch similarities: {}", e))?;

        Ok(results.into_iter()
            .map(|r| (r.target_code, r.cosine_similarity))
            .collect())
    }
}
```

### 4.4 Update: `rust/src/dsl_v2/validation.rs` (Add New Error Codes)

Add these variants to the `DiagnosticCode` enum:

```rust
// Add to DiagnosticCode enum:

    // CSG Context Errors (C0xx)
    /// Document type not applicable to entity type
    DocumentNotApplicableToEntityType,  // C001
    /// Document type not applicable to jurisdiction  
    DocumentNotApplicableToJurisdiction, // C002
    /// Document type not applicable to client type
    DocumentNotApplicableToClientType,   // C003
    /// Attribute not applicable to entity type
    AttributeNotApplicableToEntityType,  // C004
    /// Missing prerequisite operation
    MissingPrerequisiteOperation,        // C005
    /// Symbol type mismatch
    SymbolTypeMismatch,                  // C006
    /// Forward reference to undefined symbol
    ForwardReferenceError,               // C007
    /// Internal error
    InternalError,                       // C099

// Add to as_str() impl:
    DiagnosticCode::DocumentNotApplicableToEntityType => "C001",
    DiagnosticCode::DocumentNotApplicableToJurisdiction => "C002",
    DiagnosticCode::DocumentNotApplicableToClientType => "C003",
    DiagnosticCode::AttributeNotApplicableToEntityType => "C004",
    DiagnosticCode::MissingPrerequisiteOperation => "C005",
    DiagnosticCode::SymbolTypeMismatch => "C006",
    DiagnosticCode::ForwardReferenceError => "C007",
    DiagnosticCode::InternalError => "C099",
```

### 4.5 Update: `rust/src/dsl_v2/mod.rs`

```rust
// Add to module declarations:
pub mod applicability_rules;
pub mod csg_linter;
pub mod semantic_context;

// Add to re-exports:
pub use applicability_rules::{ApplicabilityRules, DocumentApplicability, AttributeApplicability};
pub use csg_linter::{CsgLinter, LintResult, InferredContext};
pub use semantic_context::SemanticContextStore;
```

---

## 5. Seed Data & Migrations

### 5.1 Run Order

Execute migrations in order:
1. `001_csg_document_types_metadata.sql`
2. `002_csg_attribute_registry_metadata.sql`
3. `003_csg_entity_types_metadata.sql`
4. `004_csg_cbus_metadata.sql`
5. `005_csg_validation_rules_table.sql`
6. `006_csg_similarity_cache.sql`

### 5.2 Seed: Document Type Applicability

```sql
-- File: sql/seeds/008_csg_document_applicability.sql

BEGIN;

-- Identity documents (person only)
UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "category": "IDENTITY",
        "subcategory": "GOVERNMENT_ISSUED",
        "purpose": "Verify identity of natural person",
        "synonyms": ["ID", "identification document"],
        "keywords": ["identity", "photo", "government"]
    }'::jsonb
WHERE type_code = 'PASSPORT';

UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "category": "IDENTITY",
        "subcategory": "GOVERNMENT_ISSUED",
        "purpose": "Verify identity via driving license",
        "synonyms": ["driving licence", "license"],
        "keywords": ["identity", "photo", "driving"]
    }'::jsonb
WHERE type_code = 'DRIVERS_LICENSE';

UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "category": "IDENTITY",
        "subcategory": "GOVERNMENT_ISSUED",
        "purpose": "Verify identity via national ID card",
        "synonyms": ["national identity card", "ID card"],
        "keywords": ["identity", "photo", "government", "national"]
    }'::jsonb
WHERE type_code = 'NATIONAL_ID';

-- Corporate formation documents
UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"],
        "required_for": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "category": "FORMATION",
        "subcategory": "INCORPORATION",
        "purpose": "Prove legal formation of company",
        "synonyms": ["incorporation certificate", "certificate of formation"],
        "keywords": ["formation", "incorporation", "company", "legal"]
    }'::jsonb
WHERE type_code = 'CERT_INCORPORATION';

UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "category": "FORMATION",
        "subcategory": "CONSTITUTIONAL",
        "purpose": "Define company governance rules",
        "synonyms": ["bylaws", "memorandum", "constitution"],
        "keywords": ["governance", "rules", "constitution", "company"]
    }'::jsonb
WHERE type_code = 'ARTICLES_ASSOC';

-- Trust documents
UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["TRUST", "TRUST_DISCRETIONARY", "TRUST_FIXED_INTEREST"],
        "required_for": ["TRUST"],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "category": "FORMATION",
        "subcategory": "TRUST",
        "purpose": "Establish trust and define terms",
        "synonyms": ["trust agreement", "declaration of trust"],
        "keywords": ["trust", "deed", "settlor", "trustee", "beneficiary"]
    }'::jsonb
WHERE type_code = 'TRUST_DEED';

-- Partnership documents
UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["PARTNERSHIP", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP", "PARTNERSHIP_GENERAL"],
        "required_for": ["PARTNERSHIP"],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "category": "FORMATION",
        "subcategory": "PARTNERSHIP",
        "purpose": "Define partnership terms and ownership",
        "synonyms": ["LPA", "partnership deed"],
        "keywords": ["partnership", "partners", "agreement", "ownership"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_AGREEMENT';

-- Universal documents (no entity restriction)
UPDATE "ob-poc".document_types SET 
    applicability = '{
        "category": "ADDRESS"
    }'::jsonb,
    semantic_context = '{
        "category": "ADDRESS",
        "subcategory": "PROOF",
        "purpose": "Verify residential or business address",
        "synonyms": ["address verification", "residence proof"],
        "keywords": ["address", "residence", "proof", "utility"]
    }'::jsonb
WHERE type_code = 'PROOF_ADDRESS';

UPDATE "ob-poc".document_types SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_*", "PARTNERSHIP_*", "TRUST_*", "LLC"],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "category": "FINANCIAL",
        "subcategory": "AUDIT",
        "purpose": "Show financial position and health",
        "synonyms": ["accounts", "annual report", "audited accounts"],
        "keywords": ["financial", "statements", "audit", "accounts"]
    }'::jsonb
WHERE type_code = 'FINANCIAL_STATEMENTS';

UPDATE "ob-poc".document_types SET 
    applicability = '{
        "required_for": ["LIMITED_COMPANY", "TRUST", "PARTNERSHIP"],
        "category": "COMPLIANCE"
    }'::jsonb,
    semantic_context = '{
        "category": "COMPLIANCE",
        "subcategory": "UBO",
        "purpose": "Declare ultimate beneficial owners",
        "synonyms": ["UBO declaration", "beneficial owner form"],
        "keywords": ["beneficial", "owner", "UBO", "declaration"]
    }'::jsonb
WHERE type_code = 'BENEFICIAL_OWNER_CERT';

COMMIT;
```

### 5.3 Seed: Entity Type Hierarchy

```sql
-- File: sql/seeds/009_csg_entity_type_hierarchy.sql

BEGIN;

-- Set up parent relationships and hierarchy paths
UPDATE "ob-poc".entity_types SET 
    type_hierarchy_path = ARRAY['ENTITY'],
    semantic_context = '{"category": "BASE", "is_abstract": true}'::jsonb
WHERE type_code = 'ENTITY';

UPDATE "ob-poc".entity_types SET 
    type_hierarchy_path = ARRAY['ENTITY', 'PERSON'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "typical_documents": ["PASSPORT", "DRIVERS_LICENSE", "NATIONAL_ID", "PROOF_ADDRESS"],
        "typical_attributes": ["full_name", "date_of_birth", "nationality", "address"]
    }'::jsonb
WHERE type_code IN ('PERSON', 'PROPER_PERSON', 'PROPER_PERSON_NATURAL');

UPDATE "ob-poc".entity_types SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY'],
    semantic_context = '{
        "category": "CORPORATE",
        "typical_documents": ["CERT_INCORPORATION", "ARTICLES_ASSOC", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["company_name", "registration_number", "incorporation_date", "jurisdiction"]
    }'::jsonb
WHERE type_code IN ('LIMITED_COMPANY', 'LLC');

UPDATE "ob-poc".entity_types SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "typical_documents": ["PARTNERSHIP_AGREEMENT", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["partnership_name", "formation_date", "partnership_type"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP';

UPDATE "ob-poc".entity_types SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST'],
    semantic_context = '{
        "category": "TRUST",
        "typical_documents": ["TRUST_DEED", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["trust_name", "formation_date", "governing_law"]
    }'::jsonb
WHERE type_code = 'TRUST';

UPDATE "ob-poc".entity_types SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'FOUNDATION'],
    semantic_context = '{
        "category": "FOUNDATION",
        "typical_documents": ["CERT_INCORPORATION", "ARTICLES_ASSOC"],
        "typical_attributes": ["foundation_name", "formation_date", "purpose"]
    }'::jsonb
WHERE type_code = 'FOUNDATION';

COMMIT;
```

---

## 6. Testing Strategy

### 6.1 Unit Tests (Rust)

```rust
// tests/csg_linter_tests.rs

#[cfg(test)]
mod tests {
    use ob_poc::dsl_v2::{parse_program, CsgLinter, ValidationContext};
    use sqlx::PgPool;

    async fn setup_test_linter() -> CsgLinter {
        let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        let mut linter = CsgLinter::new(pool);
        linter.initialize().await.unwrap();
        linter
    }

    #[tokio::test]
    async fn test_passport_for_company_rejected() {
        let linter = setup_test_linter().await;
        
        let dsl = r#"
            (entity.create-limited-company :name "Acme Corp" :as @company)
            (document.catalog :document-type "PASSPORT" :entity-id @company :cbu-id @cbu)
        "#;
        
        let ast = parse_program(dsl).unwrap();
        let result = linter.lint(ast, &ValidationContext::default(), dsl).await;
        
        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|d| 
            d.code == DiagnosticCode::DocumentNotApplicableToEntityType
        ));
    }

    #[tokio::test]
    async fn test_passport_for_person_accepted() {
        let linter = setup_test_linter().await;
        
        let dsl = r#"
            (entity.create-proper-person :first-name "John" :last-name "Doe" :as @person)
            (document.catalog :document-type "PASSPORT" :entity-id @person :cbu-id @cbu)
        "#;
        
        let ast = parse_program(dsl).unwrap();
        let result = linter.lint(ast, &ValidationContext::default(), dsl).await;
        
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn test_undefined_symbol_detected() {
        let linter = setup_test_linter().await;
        
        let dsl = r#"
            (document.catalog :document-type "PASSPORT" :entity-id @nonexistent :cbu-id @cbu)
        "#;
        
        let ast = parse_program(dsl).unwrap();
        let result = linter.lint(ast, &ValidationContext::default(), dsl).await;
        
        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|d| 
            d.code == DiagnosticCode::UndefinedSymbol
        ));
    }
}
```

### 6.2 Integration Tests (SQL)

```sql
-- tests/test_applicability_rules.sql

-- Test: Passport should apply to PROPER_PERSON
DO $$
DECLARE
    v_applies boolean;
BEGIN
    SELECT 
        (applicability->'entity_types') ? 'PROPER_PERSON'
        OR EXISTS (
            SELECT 1 FROM jsonb_array_elements_text(applicability->'entity_types') et
            WHERE 'PROPER_PERSON' LIKE REPLACE(et, '*', '%')
        )
    INTO v_applies
    FROM "ob-poc".document_types
    WHERE type_code = 'PASSPORT';
    
    ASSERT v_applies, 'PASSPORT should apply to PROPER_PERSON';
END $$;

-- Test: CERT_INCORPORATION should NOT apply to PROPER_PERSON
DO $$
DECLARE
    v_applies boolean;
BEGIN
    SELECT 
        (applicability->'entity_types') ? 'PROPER_PERSON'
    INTO v_applies
    FROM "ob-poc".document_types
    WHERE type_code = 'CERT_INCORPORATION';
    
    ASSERT NOT v_applies, 'CERT_INCORPORATION should NOT apply to PROPER_PERSON';
END $$;
```

---

## 7. Execution Checklist

### For Claude Code Agent

Execute in order. Check off each step.

```
## Phase 1: Database Migrations

[ ] 1.1 Run migration: 001_csg_document_types_metadata.sql
[ ] 1.2 Run migration: 002_csg_attribute_registry_metadata.sql  
[ ] 1.3 Run migration: 003_csg_entity_types_metadata.sql
[ ] 1.4 Run migration: 004_csg_cbus_metadata.sql
[ ] 1.5 Run migration: 005_csg_validation_rules_table.sql
[ ] 1.6 Run migration: 006_csg_similarity_cache.sql
[ ] 1.7 Verify all columns exist: 
      SELECT column_name FROM information_schema.columns 
      WHERE table_schema = 'ob-poc' AND table_name = 'document_types';

## Phase 2: Seed Data

[ ] 2.1 Run seed: 008_csg_document_applicability.sql
[ ] 2.2 Run seed: 009_csg_entity_type_hierarchy.sql
[ ] 2.3 Verify seed data:
      SELECT type_code, applicability->>'entity_types' 
      FROM "ob-poc".document_types WHERE applicability IS NOT NULL;

## Phase 3: Rust Implementation

[ ] 3.1 Create file: rust/src/dsl_v2/applicability_rules.rs
[ ] 3.2 Create file: rust/src/dsl_v2/semantic_context.rs
[ ] 3.3 Create file: rust/src/dsl_v2/csg_linter.rs
[ ] 3.4 Update file: rust/src/dsl_v2/validation.rs (add DiagnosticCodes)
[ ] 3.5 Update file: rust/src/dsl_v2/mod.rs (add module exports)
[ ] 3.6 Run: cargo check --features database
[ ] 3.7 Run: cargo test csg

## Phase 4: Integration

[ ] 4.1 Update SemanticValidator to use CsgLinter
[ ] 4.2 Run full test suite: cargo test
[ ] 4.3 Run integration test with real DB

## Phase 5: Verification

[ ] 5.1 Test: Passport rejected for company
[ ] 5.2 Test: Passport accepted for person
[ ] 5.3 Test: Undefined symbol detected
[ ] 5.4 Test: Unused symbol warning
[ ] 5.5 Verify error messages include suggestions
```

---

## Appendix: Error Message Examples

### C001: Document Not Applicable to Entity Type
```
error[C001]: document type not applicable to entity type
 --> input:3:37
  |
3 | (document.catalog :document-type "PASSPORT" :entity-id @company)
  |                                  ^^^^^^^^^^
  |
  = note: "PASSPORT" is only valid for: PROPER_PERSON, PROPER_PERSON_NATURAL
  = note: entity @company has type: LIMITED_COMPANY
  = help: valid document types for LIMITED_COMPANY:
          CERT_INCORPORATION, ARTICLES_ASSOC, FINANCIAL_STATEMENTS
```

### C006: Symbol Type Mismatch
```
error[C006]: symbol type mismatch
 --> input:5:45
  |
5 | (cbu.assign-role :cbu-id @cbu :entity-id @company :role "Director")
  |                                          ^^^^^^^^
  |
  = note: argument 'entity-id' expects PROPER_PERSON for role 'Director'
  = note: symbol @company has type: LIMITED_COMPANY
```

---

*End of Specification*
