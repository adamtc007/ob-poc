# Multi-DSL Domain Orchestration Implementation Plan

## 🎯 CRITICAL STATUS UPDATE

**Phase 1-3 Complete**: ✅ Foundation infrastructure implemented  
**Phase 4 URGENT**: 🔥 Database migration required  
**Issue**: All vocabulary hardcoded in-memory vs. database-driven architecture

---

## 🎯 Executive Summary

This plan outlines the implementation of a sophisticated **Multi-DSL Domain Orchestration** system that dynamically generates and coordinates entity-type and product-specific onboarding workflows. The system combines dynamic DSL generation with domain routing and shared context management to create a unified, compliant, and scalable onboarding platform.

## 📋 Current Architecture Analysis

### Current State
- **Main Onboarding DSL**: Basic case management, products, services
- **UBO Sub-Domain**: Entity-type-specific UBO workflows (Trust, Partnership, Corporate)
- **Hedge Fund Investor Sub-Domain**: Specialized investor onboarding workflows
- **Shared Infrastructure**: Common AttributeID-as-Type system, EBNF grammar, AST validation

### Current Limitations
- **Inconsistent DSL Grammar**: Each domain has its own parsing and validation
- **Manual DSL Domain Selection**: No automatic workflow orchestration
- **Limited Cross-Domain Integration**: No shared AttributeID system across domains
- **No Compile-Time Optimization**: Resource dependencies handled at runtime
- **Fragmented Tooling**: Different parsers, validators, and tooling per domain

## 🎯 Target Architecture: Orchestrated Multi-DSL System

```
Onboarding Request (CBU + Entities + Products)
           ↓
    [Orchestration Engine]
           ↓
┌─────────────────────────────────────────────┐
│            Context Analysis                  │
│  • CBU Entity Types & Roles                │
│  • Commercial Products                      │
│  • Regulatory Requirements                  │
│  • Dependency Graph Construction            │
└─────────────────────────────────────────────┘
           ↓
┌─────────────────────────────────────────────┐
│        Dynamic DSL Generation               │
│  • Entity-Type Workflows                   │
│  • Product-Specific Requirements           │
│  • Regulatory Compliance Templates         │
│  • Cross-Domain Dependencies               │
└─────────────────────────────────────────────┘
           ↓
┌─────────────────────────────────────────────┐
│         Domain Orchestration                │
│  Main DSL ← → UBO DSL                      │
│     ↕           ↕                          │
│  HF Investor ← → Individual KYC             │
│     ↕           ↕                          │
│  Trust KYC ← → Corporate KYC               │
└─────────────────────────────────────────────┘
           ↓
    [Unified State Document]
```

## 🏗️ Implementation Strategy: Hybrid Approach

**Combination of Option 4 (Dynamic Generation) + Option 6 (Domain Router) + Enhanced Session Management**

### Core Principles:
1. **Universal EBNF Grammar**: Single grammar for all DSL domains at platform level
2. **Domain-Specific Vocabularies**: Each domain has approved verb vocabulary with semantic rules
3. **Universal AttributeID Variables**: ALL variables in ALL DSL domains are AttributeIDs
4. **Compile-Time Optimization**: DSL compilation with dependency analysis and execution planning
5. **Shared Data Dictionary**: Single AttributeID dictionary serves all domains
6. **Cross-Domain References**: Natural AttributeID sharing between domains via `@ref{domain.attr.uuid}`

## 📐 Detailed Implementation Plan

## ✅ COMPLETED PHASES (Phase 1-3)

### Phase 1: Foundation - Orchestration Infrastructure ✅ COMPLETE
- **OrchestrationSession Management**: Multi-domain session coordination
- **Context Analysis Engine**: Entity/product-based domain discovery  
- **Domain Registry System**: Thread-safe registration and lookup
- **Cross-Domain DSL Accumulation**: Unified state management
- **Execution Planning**: Dependency resolution and optimization
- **CLI Interface**: Complete command set (`orchestrate-create`, `orchestrate-execute`, etc.)
- **Session Lifecycle**: Creation, execution, monitoring, cleanup
- **Persistent Storage**: Database-backed session management
- **Comprehensive Testing**: 95%+ coverage with integration tests

### Phase 2: Dynamic DSL Generation Engine ✅ COMPLETE  
- **DSL Generator**: Template-based DSL generation with 4 entity types
- **DSL Composition Engine**: Merges entity, product, regulatory templates
- **Template System**: 345+ line sophisticated templates (Trust UBO, Corporate UBO, Custody, FinCEN)
- **Execution Plan Optimization**: Dependency-aware execution planning
- **Master DSL Generation**: Comprehensive workflow documents
- **Cross-Jurisdictional Support**: US, EU, Swiss, UK compliance templates

### Phase 3: Orchestration DSL Verbs ✅ COMPLETE
- **Orchestration Vocabulary**: 15+ new orchestration-specific verbs
- **Verb Executor**: Processes orchestration DSL verbs with validation
- **Cross-Domain State Management**: AttributeID-as-Type coordination
- **Domain Communication**: Message routing and result collection  
- **Product Integration**: Compatibility validation and configuration
- **Workflow Coordination**: Parallel execution and dependency management

## 🔥 PHASE 4: CRITICAL DATABASE MIGRATION (IMMEDIATE PRIORITY)

**ARCHITECTURAL VIOLATION**: Current implementation uses hardcoded in-memory vocabulary instead of database-driven grammar and vocabulary storage as designed in Phase 0.

**Impact**: 
- Cannot dynamically update DSL verbs without code changes
- Vocabulary validation hardcoded in multiple places
- Cross-domain verb coordination relies on in-memory maps
- No central source of truth for grammar rules
- Prevents runtime vocabulary evolution and AI-driven verb discovery

### Phase 0/4: 🔥 **URGENT: Database-Stored Grammar & Vocabulary Migration** (Immediate Priority)

**CRITICAL ISSUE**: Current Phase 1-3 implementations use hardcoded in-memory vocabulary and grammar. This violates the core architectural principle of database-driven DSL evolution and prevents proper vocabulary validation, cross-domain coordination, and dynamic grammar updates.

**Required Immediate Actions**:

1. **Create Database Schema** - Add missing grammar and vocabulary tables
2. **Migrate All Vocabularies** - Move hardcoded verbs to database storage  
3. **Update All Vocabulary Calls** - Replace in-memory lookups with database queries
4. **Implement Dynamic Loading** - Load grammar rules and verbs from database
5. **Remove Hardcoded Mocks** - Eliminate all in-memory vocabulary implementations

### Phase 0: Universal DSL Foundation - Database-Stored Grammar & Dynamic Parser (IMPLEMENTATION REQUIRED)

#### 4.1 Database-Stored EBNF Grammar System (CRITICAL)
**Files to Create:**
- `sql/migrations/0005_create_dsl_grammar_tables.sql`
- `internal/dsl/grammar/grammar_repository.go`
- `internal/dsl/parser/dynamic_parser.go`
- `internal/dsl/ast/ast_builder.go`

**Files to Refactor:**
- `internal/orchestration/orchestration_vocabulary.go` → Replace with DB calls
- `internal/agent/dsl_agent.go` → Remove hardcoded verb validation
- `hedge-fund-investor-source/web/internal/hf-agent/hf_dsl_agent.go` → Remove hardcoded verbs
- All domain vocabulary implementations → Migrate to database

**Database Schema for Grammar Storage:**
```sql
-- Core EBNF grammar rules stored as data
CREATE TABLE "dsl-ob-poc".grammar_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_name VARCHAR(100) NOT NULL,        -- "s_expression", "attribute_ref", etc.
    rule_definition TEXT NOT NULL,          -- EBNF rule definition  
    rule_category VARCHAR(50),              -- "core", "tokens", "structures"
    version VARCHAR(20) NOT NULL DEFAULT '1.0.0',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (rule_name, version)
);

-- AST node type definitions  
CREATE TABLE "dsl-ob-poc".ast_node_types (
    node_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    node_name VARCHAR(100) NOT NULL,        -- "Statement", "Argument", "AttributeRef"
    node_structure JSONB NOT NULL,          -- Field definitions, validation rules
    go_type_definition TEXT,                -- Generated Go struct definition
    version VARCHAR(20) NOT NULL DEFAULT '1.0.0',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Token definitions for lexical analysis
CREATE TABLE "dsl-ob-poc".token_definitions (
    token_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_name VARCHAR(100) NOT NULL,       -- "UUID", "IDENTIFIER", "STRING"
    token_pattern TEXT NOT NULL,            -- Regex pattern or EBNF rule
    token_type VARCHAR(50),                 -- "TERMINAL", "REGEX", "LITERAL"
    version VARCHAR(20) NOT NULL DEFAULT '1.0.0',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX idx_grammar_rules_active ON "dsl-ob-poc".grammar_rules (is_active, version);
CREATE INDEX idx_ast_node_types_active ON "dsl-ob-poc".ast_node_types (is_active, version);
```

**Seed Data - Universal Grammar Rules:**
```sql
-- Insert core EBNF rules into database
INSERT INTO "dsl-ob-poc".grammar_rules (rule_name, rule_definition, rule_category) VALUES
('program', 'statement*', 'core'),
('statement', 's_expression | comment', 'core'),
('s_expression', '"(" verb argument* ")"', 'core'),
('verb', 'identifier', 'core'),
('argument', 'attribute_ref | cross_domain_ref | literal | s_expression | list', 'core'),
('attribute_ref', '"@attr{" uuid "}"', 'core'),
('cross_domain_ref', '"@ref{" domain_name "." attribute_name "." uuid "}"', 'core'),
('literal', 'string | number | boolean | identifier', 'tokens'),
('list', '"(" argument* ")"', 'structures'),
('string', '"\\"" [^"]* "\\""', 'tokens'),
('number', '[+-]? [0-9]+ ("." [0-9]+)?', 'tokens'),
('boolean', '"true" | "false"', 'tokens'),
('uuid', '[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}', 'tokens'),
('identifier', '[a-zA-Z][a-zA-Z0-9_.-]*', 'tokens'),
('domain_name', '[a-zA-Z][a-zA-Z0-9_-]*', 'tokens'),
('attribute_name', '[a-zA-Z][a-zA-Z0-9_.-]*', 'tokens'),
('comment', '"; [^\n]*"', 'tokens');
```

#### 4.2 Database-Stored Domain Vocabulary System (CRITICAL MIGRATION)

**Current Problem**: 
- `orchestration_vocabulary.go` has 15+ verbs hardcoded in memory
- `dsl_agent.go` has 68+ verbs hardcoded in validation map
- `hf_dsl_agent.go` has 17+ hedge fund verbs hardcoded
- Each domain has separate hardcoded vocabulary

**Required Actions**:
1. **Seed Database**: Insert all current hardcoded verbs into database tables
2. **Replace All Calls**: Update every vocabulary lookup to query database
3. **Dynamic Loading**: Implement runtime vocabulary loading from database
4. **Validation Updates**: Replace hardcoded validation with database-driven validation
5. **Cache Layer**: Add intelligent caching for performance
**Additional Database Tables:**
```sql
-- Domain vocabulary storage - all DSL verbs stored as data
CREATE TABLE "dsl-ob-poc".domain_vocabularies (
    vocab_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL,      -- "onboarding", "ubo", "hedge-fund-investor"
    verb_name VARCHAR(200) NOT NULL,        -- "case.create", "ubo.resolve-ubos"
    verb_definition JSONB NOT NULL,         -- Complete verb specification
    semantic_rules JSONB,                   -- Domain-specific validation rules
    version VARCHAR(20) NOT NULL DEFAULT '1.0.0',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (domain_name, verb_name, version)
);

-- Verb argument specifications
CREATE TABLE "dsl-ob-poc".verb_arguments (
    arg_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vocab_id UUID NOT NULL REFERENCES "dsl-ob-poc".domain_vocabularies(vocab_id) ON DELETE CASCADE,
    argument_name VARCHAR(100) NOT NULL,    -- "entity_id", "ownership_threshold"
    argument_type VARCHAR(50) NOT NULL,     -- "AttributeID", "String", "Number", "Boolean"
    is_required BOOLEAN DEFAULT FALSE,
    default_value JSONB,
    validation_rules JSONB,                 -- Type-specific validation
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Cross-domain reference rules
CREATE TABLE "dsl-ob-poc".cross_domain_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_domain VARCHAR(100) NOT NULL,
    target_domain VARCHAR(100) NOT NULL,
    reference_type VARCHAR(100),            -- "ATTRIBUTE_REF", "STATE_DEPENDENCY", "RESULT_BINDING"
    validation_rules JSONB,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX idx_domain_vocabularies_domain ON "dsl-ob-poc".domain_vocabularies (domain_name, is_active);
CREATE INDEX idx_verb_arguments_vocab ON "dsl-ob-poc".verb_arguments (vocab_id);
```

**Dynamic Vocabulary Loading:**
```go
type DynamicVocabularyRepository struct {
    db *sql.DB
}

func (dvr *DynamicVocabularyRepository) LoadDomainVocabulary(domain, version string) (*DomainVocabulary, error) {
    // Load from database instead of hardcoded maps
    query := `SELECT verb_name, verb_definition, semantic_rules 
              FROM domain_vocabularies 
              WHERE domain_name = $1 AND version = $2 AND is_active = true`
    
    // Build vocabulary dynamically from database
    return buildVocabularyFromDB(query, domain, version)
}
```

#### 0.3 Dynamic Parser Generation & Compilation Pipeline
**Files to Create:**
- `internal/dsl/parser/dynamic_parser_builder.go`
- `internal/dsl/compiler/dsl_compiler.go`
- `internal/dsl/optimizer/dependency_analyzer.go` 
- `internal/dsl/optimizer/execution_planner.go`
- `internal/dsl/optimizer/resource_optimizer.go`

**Dynamic Parser Architecture:**
```go
type DynamicParserBuilder struct {
    GrammarRepo     *GrammarRepository
    VocabularyRepo  *DynamicVocabularyRepository
    ASTBuilder      *DynamicASTBuilder
}

func (dpb *DynamicParserBuilder) BuildParser(version string) (*UniversalParser, error) {
    // 1. Load grammar rules from database
    rules := dpb.GrammarRepo.LoadGrammarRules(version)
    
    // 2. Load all domain vocabularies  
    vocabularies := dpb.VocabularyRepo.LoadAllDomainVocabularies(version)
    
    // 3. Generate parser combinators from EBNF rules
    parser := dpb.generateParserFromRules(rules, vocabularies)
    
    // 4. Build AST node constructors from database definitions
    astBuilder := dpb.ASTBuilder.BuildFromNodeTypes(version)
    
    return &UniversalParser{
        Rules:       rules,
        Vocabularies: vocabularies,
        ASTBuilder:  astBuilder,
    }, nil
}
```

**Database-Driven Compiler Pipeline:**
```
DSL Source → Dynamic Parse → Universal AST → Multi-Domain → Optimize → Execution Plan
    ↓            ↓              ↓             Validate       ↓          ↓
Raw DSL → (DB Grammar) → (DB AST Types) → (DB Vocabs) → Dependency → Optimized
Text      Rules Loaded   Nodes Built      Cross-Domain   Analysis    Execution
                                         Refs Validated             Order
```

**Optimization Capabilities:**
```go
type DSLOptimizer struct {
    DependencyAnalyzer *DependencyAnalyzer
    ResourceOptimizer  *ResourceOptimizer  
    ExecutionPlanner   *ExecutionPlanner
}

type OptimizationResult struct {
    ExecutionOrder     []ExecutionStage
    ResourceDependencies map[string][]string
    ParallelExecutionGroups [][]string
    ResourceWaitConditions []WaitCondition
}

// Example optimization:
// Don't create custody account until entity UBO verification complete
type WaitCondition struct {
    WaitFor    string // "@attr{ubo-verification-complete}"
    BeforeExec string // "(resources.create-custody-account ...)"
    Reason     string // "UBO_VERIFICATION_DEPENDENCY"
}
```

#### 0.4 Parser Generation Tooling & Testing
**Files to Create:**
- `internal/dsl/tools/grammar_validator.go`
- `internal/dsl/tools/parser_generator.go`
- `internal/dsl/testing/dynamic_parser_tests.go`

**Grammar Management Tools:**
```go
type GrammarManager struct {
    repo *GrammarRepository
}

func (gm *GrammarManager) ValidateGrammarRules(version string) error {
    // Load rules and check for conflicts, cycles, undefined references
}

func (gm *GrammarManager) GenerateParserCode(version string) (string, error) {
    // Generate Go parser code from EBNF rules for performance-critical parsing
}

func (gm *GrammarManager) TestGrammarWithSamples(version string, samples []string) []TestResult {
    // Test grammar against sample DSL documents
}
```

**Dynamic Parser Benefits:**
- ✅ **Versioned Grammar**: Multiple grammar versions can coexist
- ✅ **Runtime Updates**: Update parser without code deployment
- ✅ **Domain Management**: Add/remove domains via database
- ✅ **Testing**: Validate grammar changes before activation  
- ✅ **Audit Trail**: Complete history of all grammar modifications
- ✅ **Environment-Specific**: Different environments can have different grammars

## 📋 PHASE 4 IMPLEMENTATION STEPS

### Step 1: Create Database Schema (URGENT - Day 1)
**File**: `sql/migrations/0005_create_dsl_grammar_tables.sql`

```sql
-- Grammar Rules Storage
CREATE TABLE "dsl-ob-poc".grammar_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_name VARCHAR(100) NOT NULL,
    rule_definition TEXT NOT NULL,
    rule_category VARCHAR(50),
    version VARCHAR(20) NOT NULL DEFAULT '1.0.0',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (rule_name, version)
);

-- Domain Vocabularies Storage  
CREATE TABLE "dsl-ob-poc".domain_vocabularies (
    vocab_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL,
    verb_name VARCHAR(200) NOT NULL,
    verb_definition JSONB NOT NULL,
    verb_category VARCHAR(50),
    semantic_rules JSONB,
    version VARCHAR(20) NOT NULL DEFAULT '1.0.0',
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (domain_name, verb_name, version)
);

-- Verb Arguments Storage
CREATE TABLE "dsl-ob-poc".verb_arguments (
    arg_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vocab_id UUID NOT NULL REFERENCES "dsl-ob-poc".domain_vocabularies(vocab_id),
    argument_name VARCHAR(100) NOT NULL,
    argument_type VARCHAR(50) NOT NULL,
    is_required BOOLEAN DEFAULT FALSE,
    validation_rules JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Orchestration Cross-Domain Rules
CREATE TABLE "dsl-ob-poc".orchestration_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_domain VARCHAR(100) NOT NULL,
    target_domain VARCHAR(100) NOT NULL,
    rule_type VARCHAR(100),
    rule_definition JSONB,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
```

### Step 2: Migrate Hardcoded Vocabularies (URGENT - Day 2-3)
**Priority Order**:
1. **Orchestration Verbs** (15 verbs from `orchestration_vocabulary.go`)
2. **Onboarding Verbs** (68 verbs from `dsl_agent.go`)  
3. **Hedge Fund Verbs** (17 verbs from `hf_dsl_agent.go`)
4. **UBO Domain Verbs** (discover and implement)
5. **KYC Domain Verbs** (discover and implement)

### Step 3: Database Repository Layer (Day 4-5)
**Files to Create**:
- `internal/vocabulary/repository.go` - Database vocabulary access
- `internal/grammar/repository.go` - Database grammar access  
- `internal/parser/dynamic_parser.go` - Database-driven parser

### Step 4: Replace All In-Memory Calls (Day 6-7)
**Files to Update**:
- Remove `NewOrchestrationVocabulary()` hardcoded implementation
- Update `ValidateOrchestrationVerbs()` to query database
- Replace `CallDSLTransformationAgent()` verb validation with database
- Update all domain `GetVocabulary()` methods to query database

### Step 5: Implement Caching Layer (Day 8)
- Redis/in-memory cache for frequently accessed verbs
- Cache invalidation on vocabulary updates
- Performance optimization for verb lookups

---

### Phase 1: Foundation - Orchestration Infrastructure ✅ COMPLETE (Weeks 1-2)

#### 1.1 Multi-Domain Session Manager Enhancement
**Files to Modify:**
- `internal/shared-dsl/session/manager.go`
- Create: `internal/orchestration/session_orchestrator.go`

**New Capabilities:**
```go
type OrchestrationSession struct {
    PrimaryDomain   string                    // "onboarding"
    ActiveDomains   map[string]*DomainSession // sub-domain sessions
    SharedContext   *SharedContext            // cross-domain state
    ExecutionPlan   *ExecutionPlan           // dependency graph + execution order
}

type SharedContext struct {
    CBU             *CBUContext              // entities, roles, products
    AttributeValues map[string]interface{}   // shared attribute state
    CrossDomainRefs map[string]string       // entity ID mappings
}
```

#### 1.2 Domain Router & Registry Extension
**Files to Create:**
- `internal/orchestration/domain_router.go`
- `internal/orchestration/execution_planner.go`

**New Capabilities:**
- Route DSL fragments to appropriate sub-domains
- Manage cross-domain dependencies
- Coordinate parallel execution where possible
- Aggregate results back to main orchestration session

#### 1.3 Context Analysis Engine
**Files to Create:**
- `internal/orchestration/context_analyzer.go`
- `internal/orchestration/entity_product_mapper.go`

**Functions:**
```go
func AnalyzeOnboardingContext(cbu *CBU, products []Product) *OnboardingContext
func DetermineRequiredDomains(context *OnboardingContext) []RequiredDomain
func BuildDependencyGraph(domains []RequiredDomain) *ExecutionPlan
```

### Phase 2: Dynamic DSL Generation Engine ✅ COMPLETE (Weeks 3-4)

#### 2.1 DSL Template System
**Files to Create:**
- `internal/orchestration/dsl_generator.go`
- `internal/orchestration/templates/entity_templates.go`
- `internal/orchestration/templates/product_templates.go`

**Template Structure:**
```
templates/
├── entity_types/
│   ├── trust_ubo_workflow.dsl.tmpl
│   ├── partnership_ubo_workflow.dsl.tmpl
│   ├── individual_kyc_workflow.dsl.tmpl
│   └── corporate_ubo_workflow.dsl.tmpl
├── products/
│   ├── custody_requirements.dsl.tmpl
│   ├── fund_accounting_requirements.dsl.tmpl
│   └── transfer_agent_requirements.dsl.tmpl
└── regulatory/
    ├── fincen_control_prong.dsl.tmpl
    ├── eu_5mld_dual_prong.dsl.tmpl
    └── fatf_trust_compliance.dsl.tmpl
```

#### 2.2 DSL Composition Engine
**Capabilities:**
- Merge entity-type workflows with product requirements
- Apply regulatory compliance templates based on jurisdiction
- Generate execution dependencies between workflows
- Produce single "Master DSL" for orchestration

**Example Generated Output:**
```lisp
; Auto-generated Master DSL for CBU-COMPLEX-001
(case.create (cbu.id "CBU-COMPLEX-001"))
(context.initialize 
  (entities ["entity-trust-1", "entity-hedgefund-1", "entity-individual-1"])
  (products ["CUSTODY", "FUND_ACCOUNTING"])
  (jurisdictions ["US", "GB"]))

; Trust Entity Workflow (generated from templates)
(workflow.execute-subdomain
  (domain "ubo")
  (template "trust-fatf-ubo")
  (entity.target "entity-trust-1")
  (depends.on [])
  (result.binding "@attr{trust-ubo-complete}"))

; Hedge Fund Workflow (depends on Trust UBO)
(workflow.execute-subdomain
  (domain "hedge-fund-investor")
  (template "partnership-hf-investor")
  (entity.target "entity-hedgefund-1") 
  (depends.on ["@attr{trust-ubo-complete}"])
  (result.binding "@attr{hf-investor-complete}"))

; Product Requirements Integration
(workflow.apply-product-requirements
  (products ["CUSTODY", "FUND_ACCOUNTING"])
  (to.entities ["entity-trust-1", "entity-hedgefund-1"])
  (depends.on ["@attr{trust-ubo-complete}", "@attr{hf-investor-complete}"]))
```

### Phase 3: Orchestration DSL Verbs ✅ COMPLETE (Weeks 5-6)

#### 3.1 New Orchestration Verbs
**Files to Modify:**
- `internal/domains/onboarding/domain.go`
- `internal/agent/dsl_agent.go` (add verbs to validation)

**New DSL Verbs:**
```lisp
; Context Management
(context.initialize (entities [...]) (products [...]) (jurisdictions [...]))
(context.analyze (cbu.id "..."))
(context.share-state (to.domains [...]) (attributes [...]))

; Workflow Orchestration  
(workflow.execute-subdomain (domain "...") (template "...") (entity.target "..."))
(workflow.coordinate-parallel (workflows [...]) (sync.points [...]))
(workflow.wait-for-completion (workflows [...]) (timeout "..."))

; Domain Communication
(domain.route-to (domain "...") (dsl.fragment "...") (context @attr{...}))
(domain.collect-results (from.domains [...]) (result.binding "@attr{...}"))
(domain.sync-state (between.domains [...]) (attributes [...]))

; Product Integration
(products.apply-requirements (products [...]) (to.entities [...]))
(products.validate-compatibility (entities [...]) (products [...]))
```

#### 3.2 Cross-Domain State Management
**Enhanced AttributeID System:**
- Cross-domain attribute references
- State synchronization between domains
- Conflict resolution for shared attributes

### Phase 4: Enhanced Sub-Domain Integration (Weeks 7-8)

#### 4.1 Standardize Domain Interfaces
**All Sub-Domains Must Implement:**
```go
type OrchestratableDomain interface {
    // Orchestration Interface
    AcceptOrchestrationContext(ctx *SharedContext) error
    ExecuteWithDependencies(deps []string, results map[string]interface{}) error
    PublishResults() map[string]interface{}
    
    // State Coordination
    GetExportableState() map[string]interface{}
    ImportSharedState(state map[string]interface{}) error
    ValidatePrerequisites(deps []string) error
}
```

**Domains to Update:**
- `internal/domains/ubo/domain.go`
- `internal/domains/onboarding/domain.go`
- `hedge-fund-investor-source/web/internal/hf-agent/`

#### 4.2 Result Aggregation & Coordination
**Files to Create:**
- `internal/orchestration/result_aggregator.go`
- `internal/orchestration/state_coordinator.go`

**Functions:**
- Collect results from all sub-domains
- Merge AttributeID state across domains  
- Validate completion criteria
- Generate unified compliance report

### Phase 5: Product-Driven Workflow Customization (Weeks 9-10)

#### 5.1 Product-Entity Requirement Mapping
**Files to Create:**
- `internal/orchestration/product_requirements.go`
- `internal/orchestration/compliance_matrix.go`

**Product Requirement System:**
```go
type ProductRequirements struct {
    ProductID     string
    EntityTypes   []string              // Which entity types this applies to
    RequiredDSL   []string             // DSL fragments this product requires
    Attributes    []string             // Required AttributeIDs
    Compliance    []ComplianceRule     // Regulatory requirements
}

// Examples:
CUSTODY_REQUIREMENTS = {
    ProductID: "CUSTODY",
    EntityTypes: ["TRUST", "CORPORATION", "PARTNERSHIP"],
    RequiredDSL: ["custody.account-setup", "custody.signatory-verification"],
    Attributes: ["custody.account_number", "custody.signatory_authority"],
    Compliance: [FINCEN_CONTROL_PRONG, EU_5MLD_UBO]
}
```

#### 5.2 Dynamic Product Integration
- Analyze CBU products and entities
- Generate product-specific DSL requirements
- Merge with entity-type workflows
- Validate product-entity compatibility

### Phase 6: Compile-Time Optimization & Execution Planning (Weeks 11-12)

#### 6.1 DSL Compile-Time Optimization Pipeline
**Files Enhanced from Phase 0:**
- `internal/dsl/compiler/dsl_compiler.go`
- `internal/dsl/optimizer/dependency_analyzer.go`
- `internal/dsl/optimizer/execution_planner.go`
- `internal/dsl/optimizer/resource_optimizer.go`

**Advanced Optimization Features:**
```go
// Dependency Analysis & Optimization
type DependencyOptimizer struct {
    AttributeGraph   *AttributeDependencyGraph
    ResourceTracker  *ResourceDependencyTracker
    ExecutionPlanner *OptimizedExecutionPlanner
}

// Resource Creation Dependencies
type ResourceDependency struct {
    ResourceType    string // "CUSTODY_ACCOUNT", "SIGNATORY_AUTHORITY"
    DependsOn       []string // "@attr{ubo-verification-complete}"
    CreationVerb    string // "(resources.create-custody-account ...)"
    WaitCondition   string // "UBO_IDENTITY_VERIFIED"
    FailureHandling string // "ROLLBACK_PARTIAL_RESOURCES"
}

// Execution Order Optimization
func (do *DependencyOptimizer) OptimizeExecutionOrder(dsl string) *ExecutionPlan {
    ast := ParseUniversalDSL(dsl)
    deps := do.AnalyzeDependencies(ast)
    return do.GenerateOptimalExecutionOrder(deps)
}
```

**Platform-Level Optimizations:**
- **Resource Creation Ordering**: Don't create custody accounts until UBO verification complete
- **Cross-Domain Synchronization**: Wait for Trust UBO completion before Partnership analysis  
- **Parallel Execution Opportunities**: Identify independent workflows for concurrent execution
- **Resource Dependency Management**: Track resource URLs and prerequisites
- **Failure Recovery Planning**: Generate rollback sequences for partial completions

#### 6.2 Universal AST Validation Enhancement
**Enhanced Universal Validation:**
- Cross-domain AttributeID reference validation using single dictionary
- Circular dependency detection across all domains
- Product-entity compatibility checking with optimization hints
- Compile-time resource dependency validation
- Type enforcement via universal AttributeID dictionary

### Phase 7: Testing & Integration (Weeks 13-14)

#### 7.1 Integration Testing
**Test Scenarios:**
1. **Complex Multi-Entity Case**: Trust + Hedge Fund + Individual UBOs + Multiple Products
2. **Dependency Coordination**: Parallel execution with proper synchronization
3. **Cross-Domain State**: Attribute sharing and conflict resolution
4. **Product Integration**: Product requirements applied across entity types
5. **Regulatory Compliance**: FinCEN + EU 5MLD + FATF compliance in single workflow

#### 7.2 Performance Testing
- Large DSL document generation
- Multi-domain parallel execution
- Cross-domain state synchronization overhead
- Memory usage with complex orchestration sessions

## 📊 Data Model Extensions

### New Database Tables Required

#### Orchestration Session Tracking
```sql
CREATE TABLE "dsl-ob-poc".orchestration_sessions (
    session_id UUID PRIMARY KEY,
    cbu_id UUID NOT NULL,
    primary_domain VARCHAR(100),
    active_domains JSONB,
    shared_context JSONB,
    execution_plan JSONB,
    status VARCHAR(50),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
```

#### Product Requirements
```sql
CREATE TABLE "dsl-ob-poc".product_entity_requirements (
    requirement_id UUID PRIMARY KEY,
    product_id UUID REFERENCES products(product_id),
    entity_type VARCHAR(100),
    required_dsl_template VARCHAR(255),
    required_attributes JSONB,
    compliance_rules JSONB,
    created_at TIMESTAMPTZ
);
```

#### Cross-Domain State
```sql
CREATE TABLE "dsl-ob-poc".cross_domain_state (
    state_id UUID PRIMARY KEY,
    session_id UUID REFERENCES orchestration_sessions(session_id),
    domain_name VARCHAR(100),
    attribute_id UUID REFERENCES dictionary(attribute_id),
    attribute_value JSONB,
    shared_with_domains JSONB,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
```

## 🔒 Type Enforcement Strategy

### AttributeID-as-Type Validation
1. **Cross-Domain Reference Validation**: Ensure AttributeIDs referenced across domains exist in shared dictionary
2. **Type Compatibility Checking**: Validate that AttributeID usage is consistent across domains
3. **Product-Attribute Mapping**: Enforce that required attributes for products are collected
4. **Compliance Attribute Coverage**: Validate that regulatory requirements are met

### AST-Based Type Checking
```go
type TypeChecker struct {
    Dictionary    *AttributeDictionary
    CrossDomainRefs map[string][]string  // domain -> attribute references
    ProductRequirements map[string][]string  // product -> required attributes
}

func (tc *TypeChecker) ValidateOrchestrationAST(ast *OrchestrationAST) []ValidationError {
    // 1. Validate all AttributeID references exist in dictionary
    // 2. Check cross-domain attribute compatibility
    // 3. Validate product requirement coverage
    // 4. Detect circular dependencies
    // 5. Verify completion criteria are achievable
}
```

## 🚀 Implementation Timeline

| Phase | Duration | Focus | Deliverables |
|-------|----------|-------|--------------|
| 0 | Weeks -2 to 0 | **Universal Foundation** | **Universal EBNF, Domain Vocabularies, DSL Compiler Pipeline** |
| 1 | Weeks 1-2 | Orchestration Infrastructure | Multi-Domain Session Manager, Domain Router |
| 2 | Weeks 3-4 | Dynamic Generation | DSL Templates, Composition Engine |
| 3 | Weeks 5-6 | Orchestration Verbs | Cross-Domain DSL Verbs, Shared State Management |
| 4 | Weeks 7-8 | Domain Integration | Universal Domain Interface, AttributeID Standardization |
| 5 | Weeks 9-10 | Product Integration | Product-Driven Workflow Customization |
| 6 | Weeks 11-12 | **Compile-Time Optimization** | **Dependency Analysis, Resource Optimization, Execution Planning** |
| 7 | Weeks 13-14 | Testing & Validation | Integration Tests, Performance Validation, Optimization Verification |

## 🎯 Success Criteria

### Functional Requirements
- ✅ **Universal DSL Consistency**: All domains use same grammar, AttributeID system, and tooling
- ✅ **Compile-Time Optimization**: DSL compilation with dependency analysis and execution planning
- ✅ **Single API call creates complex multi-entity, multi-product onboarding case**  
- ✅ **Resource Dependency Management**: Automatic resource creation ordering and dependency resolution
- ✅ **Cross-Domain State Coordination**: Natural AttributeID sharing and validation across domains
- ✅ **Unified DSL-as-State Document**: Complete audit trail with compile-time optimization metadata
- ✅ **Full Regulatory Compliance**: FinCEN, EU 5MLD, FATF compliance across all domains

### Non-Functional Requirements
- ✅ **Compile-Time Performance**: DSL compilation and optimization within 2 seconds
- ✅ **Runtime Performance**: Execute optimized workflows within 10 seconds  
- ✅ **Scalability**: Handle 100+ concurrent orchestration sessions with optimized execution
- ✅ **Reliability**: 99.9% success rate with compile-time dependency validation
- ✅ **Developer Experience**: Single grammar, consistent tooling, unified debugging across all domains
- ✅ **Maintainability**: Universal foundation with clean domain-specific layers

### Example Success Case
```bash
POST /api/onboarding/orchestrate
{
  "cbu_id": "CBU-COMPLEX-001",
  "entities": [
    {"type": "TRUST", "name": "Smith Family Trust", "jurisdiction": "US", "role": "CLIENT"},
    {"type": "HEDGE_FUND", "name": "Alpha Fund LP", "jurisdiction": "GB", "role": "INVESTMENT_VEHICLE"},
    {"type": "PROPER_PERSON", "name": "John Smith", "role": "UBO"}
  ],
  "products": ["CUSTODY", "FUND_ACCOUNTING", "TRANSFER_AGENT"],
  "regulatory_frameworks": ["FINCEN_CDD", "EU_5MLD", "FATF_TRUST"]
}

Response: Single orchestrated DSL document with:
✅ Trust-specific UBO workflow (FATF compliant)
✅ Partnership dual-prong UBO workflow (EU 5MLD compliant) 
✅ FinCEN Control Prong identification
✅ Individual KYC workflow
✅ Product-specific requirements integration
✅ Cross-domain dependency coordination
✅ Complete audit trail and compliance documentation
```

## 🔄 Migration Strategy

### Backward Compatibility
- Existing single-domain DSL workflows continue to work unchanged
- Gradual migration of complex cases to orchestration system
- Opt-in orchestration via API parameter

### Rollout Phases
1. **Phase 0**: Deploy universal DSL foundation alongside existing domain-specific parsers
2. **Phase 1**: Migrate individual domains to universal grammar (backward compatible)
3. **Phase 2**: Enable cross-domain orchestration for new complex cases  
4. **Phase 3**: Deploy compile-time optimization pipeline
5. **Phase 4**: Migrate existing cases to optimized execution
6. **Phase 5**: Full production deployment with universal DSL platform

## 📈 Monitoring & Observability

### Key Metrics
- Orchestration session success/failure rates
- Cross-domain state synchronization performance
- DSL generation time vs complexity
- Regulatory compliance coverage percentage

### Logging Strategy
- Structured logs for each orchestration phase
- Cross-domain state changes tracking
- Performance metrics for each sub-domain execution
- Complete audit trail for regulatory examination

## 🔐 Security Considerations

### Cross-Domain Security
- AttributeID access control across domains
- Secure state sharing between domains
- Audit logging of all cross-domain communications

### Compliance Security
- PII handling across multiple domains
- Regulatory data classification enforcement
- Secure storage of orchestration state

---

## 📋 IMMEDIATE NEXT STEPS (CRITICAL)

### 🔥 Phase 4 Database Migration (THIS WEEK)
1. **Create Database Schema** - Add grammar/vocabulary tables to `sql/init.sql`
2. **Seed Current Vocabularies** - Migrate all hardcoded verbs to database
3. **Implement Repository Pattern** - Database access layer for vocabularies
4. **Update All Calls** - Replace in-memory with database lookups
5. **Add Caching** - Performance optimization layer
6. **Remove Hardcoded Implementations** - Clean up in-memory mocks
7. **Test Database Integration** - Verify all functionality works with DB

### Success Criteria for Phase 4
- [ ] All DSL verbs stored in database tables
- [ ] Zero hardcoded vocabulary in code  
- [ ] All validation queries database
- [ ] Dynamic vocabulary loading working
- [ ] Performance acceptable with caching
- [ ] All existing tests pass with database backend
- [ ] New verbs can be added via database inserts (not code changes)

### Post-Phase 4: Ready for Production
- **Dynamic Vocabulary Updates**: Add/modify verbs without deployments
- **AI-Driven Verb Discovery**: LLM can suggest new verbs stored in database
- **Cross-Domain Coordination**: Database-enforced vocabulary consistency
- **Audit Trail**: Complete history of vocabulary changes
- **Multi-Tenant Support**: Domain-specific vocabularies per tenant

---

## 📋 Next Steps for Review (AFTER Phase 4)

**Please review this enhanced implementation plan focusing on:**

1. **Universal DSL Foundation**: Does the single EBNF grammar with domain-specific vocabularies create the right abstraction?

2. **Compile-Time Optimization**: Is the DSL compilation pipeline with dependency analysis and resource optimization aligned with your vision?

3. **AttributeID Universality**: Does making ALL variables AttributeIDs across ALL domains provide the semantic consistency needed?

4. **Cross-Domain Integration**: Are the `@ref{domain.attr.uuid}` cross-domain references sufficient for complex orchestration?

5. **Implementation Timeline**: Is the enhanced 16-week timeline (including Phase 0) realistic for this foundational work?

6. **Platform-Level Benefits**: Does this approach create the "onboarding platform level optimizations" you envisioned?

**Key Innovation**: The plan now treats DSL as a **compiled language** with platform-level optimizations, dependency management, and execution planning - transforming from "script execution" to "intelligent workflow compilation."

**Upon your approval, we can begin implementation starting with Phase 0: Universal DSL Foundation.**