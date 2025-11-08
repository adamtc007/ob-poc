# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**DSL Onboarding POC** now uses a consolidated Rust DslManager as the orchestration and persistence layer. The Go code is demo-only and communicates with the Rust backend (or uses mocks). Direct database access from Go has been deprecated and removed.

## 🏗️ Core Architectural Pattern: **DSL-as-State**

**This is the fundamental pattern that makes the entire system work.**

### What is DSL-as-State?

The DSL is not just a representation of state—**the DSL IS the state itself**. This is the key architectural insight:

- **State = Accumulated DSL Document**: Each onboarding case's current state is represented by its complete, accumulated DSL document
- **Immutable Event Sourcing**: Each command (create, add-products, discover-kyc, etc.) appends to the DSL, creating a new version
- **Executable Documentation**: The DSL is simultaneously:
  - Human-readable documentation of what happened
  - Machine-parseable structured data
  - Complete audit trail for compliance
  - State reconstruction mechanism
  - Executable workflow definition

### Why This Works for Onboarding/KYC/Investor Management

1. **Compositional State Building**: Each operation builds on previous DSL
   ```lisp
   ;; Version 1: Create case
   (case.create (cbu.id "CBU-1234") (nature-purpose "UCITS fund"))
   
   ;; Version 2: Add products (accumulated)
   (case.create (cbu.id "CBU-1234") (nature-purpose "UCITS fund"))
   (products.add "CUSTODY" "FUND_ACCOUNTING")
   
   ;; Version 3: Discover KYC (accumulated)
   (case.create (cbu.id "CBU-1234") (nature-purpose "UCITS fund"))
   (products.add "CUSTODY" "FUND_ACCOUNTING")
   (kyc.start (documents (document "CertificateOfIncorporation")) ...)
   ```

2. **Complete Audit Trail**: Required for regulatory compliance—every decision is captured in the DSL

3. **State Inspection at Any Point**: Parse the DSL to understand current onboarding stage

4. **Time Travel**: Access any historical version to see state at that moment

5. **Workflow Coordination**: Multiple systems can consume the same DSL to execute their parts

6. **Validation & Testing**: Can validate entire workflows by parsing accumulated DSL

### Implementation Examples

**Main Onboarding POC** (now via Rust backend; Go examples retained for demo):
- `create` → Initial DSL version
- `add-products` → Appends to DSL
- `discover-kyc` → Appends KYC requirements
- `discover-services` → Appends service plan
- `discover-resources` → Appends resource plan
- `history` → Shows DSL evolution over time

**Hedge Fund Investor Module** (`hedge-fund-investor-source/web/`):
- `ChatSession.BuiltDSL` → Accumulated DSL throughout conversation
- Each chat message generates DSL fragment
- Fragments accumulate into complete investor onboarding DSL
- Resolves entity UUIDs (investor, fund, class) from context
- Entire conversation state visible in one DSL document

### Key Architectural Benefits

✅ **Immutability**: DSL versions never change, only accumulate
✅ **Traceability**: Complete history of all decisions
✅ **Composability**: Build complex state from simple operations
✅ **Declarative**: What to do, not how to do it
✅ **Testable**: Can verify workflows by parsing DSL
✅ **Extensible**: Add new verbs without breaking existing DSL
✅ **Human-Readable**: Business users can review the DSL
✅ **Machine-Executable**: Systems can parse and execute

### Critical Implementation Details

1. **Verb Validation**: Only approved DSL verbs are allowed (prevents AI hallucination)
2. **UUID Resolution**: Placeholders like `<investor_id>` resolved to actual UUIDs
3. **Context Maintenance**: Session/case context tracks entities across operations
4. **Accumulation**: Each operation appends, never replaces
5. **Versioning**: Each append creates new database version with full DSL

## ✅ Recent Implementation Updates

### DSL Verb Validation (Completed)
**Problem**: AI agents could generate unapproved DSL verbs, leading to hallucinated operations.

**Solution**: Implemented verb validation in both systems:
- **Main Onboarding POC** (`internal/agent/dsl_agent.go`): Added `validateDSLVerbs()` function with 70+ approved verbs
- **Hedge Fund Module** (`hedge-fund-investor-source/web/internal/hf-agent/`): Already had validation with 17 approved verbs
- Validation occurs after AI generation, before DSL is stored
- System prompt explicitly lists approved verbs as constraints
- Comprehensive test coverage (`internal/agent/dsl_agent_test.go`)

**Impact**: Prevents AI from inventing operations, ensures DSL correctness, maintains domain vocabulary integrity.

### Stateful DSL Accumulation (Completed)
**Problem**: Hedge fund chat wasn't maintaining accumulated DSL state with resolved UUIDs. Each message generated standalone DSL snippets with placeholders like `<investor_id>`.

**Solution**: Implemented accumulated DSL pattern in chat interface:
- **ChatSession.BuiltDSL** (`hedge-fund-investor-source/web/server.go`): Maintains full accumulated DSL throughout conversation
- **Context Resolution**: Tracks investor_id, fund_id, class_id, etc. across messages
- **UUID Resolution**: Replaces placeholders with actual UUIDs from context via `resolveDSLPlaceholders()`
- **ExistingDSL Context**: Passes accumulated DSL to agent for subsequent operations
- **System Prompt Updates**: Explicitly instructs AI to use actual context values instead of placeholders

**Example Flow**:
1. User: "Create opportunity for Henry Cearns" → Generates UUID, stores in context
2. User: "Start KYC" → Uses UUID from context, appends to BuiltDSL
3. User: "Collect document" → Uses same UUID, accumulates in BuiltDSL
4. Result: Complete onboarding journey visible in single DSL document

**Impact**: Chat state now mirrors main POC's versioned DSL pattern. Complete audit trail. Referential integrity across operations.

### Testing & Verification
- ✅ Verb validation in Go mocks retained for demos
- ✅ Accumulation examples kept in examples; production orchestrations run in Rust

---

## 🔑 Second Core Pattern: **AttributeID-as-Type**

**Variables in the DSL are typed by their attributeID (UUID), not by primitive types.**

### The Pattern

S-expression structure:
```lisp
(verb attributeID attributeID attributeID ...)
```

Where each **attributeID** is a **UUID** that references the **dictionary table** (universal schema).

### Example: Hedge Fund Investor

```lisp
(investor.start-opportunity
  @attr{uuid-0001}  ; → investor.legal_name (string, PII)
  @attr{uuid-0002}  ; → investor.type (enum: PROPER_PERSON|CORPORATE|TRUST)
  @attr{uuid-0003}  ; → investor.domicile (string, ISO country code)
)

(kyc.begin
  @attr{uuid-0001}  ; → Same investor.legal_name
  @attr{uuid-0004}  ; → kyc.risk_rating (enum: LOW|MEDIUM|HIGH)
)
```

### Example: Main Onboarding POC

```lisp
(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    (var (attr-id "8a5d1a77-..."))  ; → custody.account_number
  )
)

(values.bind
  (bind (attr-id "8a5d1a77-...") (value "CUSTODY-ACC-001"))
)
```

### Why AttributeID-as-Type is Powerful

1. **Metadata-Driven Type System**: All type information lives in the dictionary table
   - Data type (string, number, date, enum)
   - Validation rules
   - Privacy classification (PII, PCI, PHI)
   - Allowed values for enums
   - Source metadata (where to get the value)
   - Sink metadata (where to store the value)

2. **Late Binding**: Values can be resolved at different times from different sources
   ```
   Time 1: DSL declares (var (attr-id "uuid")) → placeholder
   Time 2: Value bound from user input → "John Smith"
   Time 3: Value enriched from CRM system → additional metadata
   ```

3. **Universal Data Contract**: AttributeID is the agreement between all systems
   - Frontend knows to collect attr-id "xyz" with specific validation
   - Backend knows to store attr-id "xyz" in specific database column
   - Compliance knows attr-id "xyz" is PII and must be encrypted
   - Analytics knows attr-id "xyz" must be masked in reports

4. **Data Governance Built-In**: Privacy and compliance at the type level
   ```sql
   SELECT attribute_id, name, mask, domain, source, sink
   FROM dictionary
   WHERE attribute_id = '8a5d1a77-...'
   -- Returns: custody.account_number, 'string', 'Settlement', {...}
   ```

5. **Cross-System Semantic Interoperability**: Different systems agree on meaning
   - Not just "this is a string" but "this is an investor legal name"
   - Not just "this is a number" but "this is a subscription amount in base currency"
   - Semantic type system vs syntactic type system

6. **Versioning and Evolution**: Attribute definitions can evolve without breaking DSL
   - Add new validation rules → existing DSL still valid
   - Change source system → DSL references unchanged
   - Migrate data stores → attributeID remains constant

### Dictionary Table Structure

```sql
-- Note: canonical schema is "ob-poc" (legacy "dsl-ob-poc" normalized)
CREATE TABLE "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY,           -- The "type" identifier
    name VARCHAR(255) NOT NULL UNIQUE,       -- Human-readable name
    long_description TEXT,                   -- For AI discovery
    group_id VARCHAR(100),                   -- Logical grouping (KYC, Settlement, etc.)
    mask VARCHAR(50) DEFAULT 'string',       -- Data type/format
    domain VARCHAR(100),                     -- Business domain
    vector TEXT,                             -- For AI semantic search
    source JSONB,                            -- SourceMetadata: where/how to get value
    sink JSONB,                              -- SinkMetadata: where/how to store value
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
```

### How It Works Together

1. **DSL declares variables** by attributeID:
   ```lisp
   (resources.plan
     (resource.create "CustodyAccount"
       (var (attr-id "8a5d1a77-..."))))
   ```

2. **Dictionary defines the attribute**:
   ```json
   {
     "attribute_id": "8a5d1a77-...",
     "name": "custody.account_number",
     "mask": "string",
     "domain": "Settlement",
     "source": {"type": "generated", "format": "CUST-{sequence}"},
     "sink": {"table": "accounts", "column": "account_number"}
   }
   ```

3. **Runtime resolution** binds the value:
   ```lisp
   (values.bind
     (bind (attr-id "8a5d1a77-...") (value "CUST-000123")))
   ```

4. **Systems consume** using attributeID as the contract:
   - DSL executor knows what to validate (from dictionary.mask)
   - Data collector knows where to get value (from dictionary.source)
   - Data persister knows where to store (from dictionary.sink)
   - Compliance knows how to protect (from dictionary metadata)

### Comparison to Traditional Approaches

| Traditional | AttributeID-as-Type |
|-------------|---------------------|
| `string accountNumber` | `@attr{uuid} → dictionary → "custody.account_number"` |
| Type = syntax (string) | Type = semantics (what it means) |
| Validation in code | Validation in dictionary metadata |
| Privacy in separate system | Privacy in attribute definition |
| Hard-coded sources | Source metadata in dictionary |
| Schema changes break code | Dictionary evolution, DSL stable |

### Real-World Benefits

✅ **Single Source of Truth**: Dictionary is the universal schema  
✅ **AI-Friendly**: LLMs can discover attributes by description  
✅ **Compliance-Ready**: Privacy flags embedded in type system  
✅ **Multi-Source**: Same attribute can come from different sources  
✅ **Auditable**: Complete provenance tracking via source metadata  
✅ **Evolvable**: Change implementation without changing DSL  
✅ **Cross-System**: All systems speak the same "type language"  

### The Two Patterns Together

**DSL-as-State** + **AttributeID-as-Type** = Complete Onboarding System

```
State = Accumulated DSL Document
DSL = S-expressions of (verb attributeID attributeID ...)
AttributeID = UUID → Dictionary (universal schema)
Dictionary = Metadata-driven type system with governance

Result: Self-describing, evolvable, auditable, compliant state machine
```

### Concrete Example: Hedge Fund Investor Onboarding

**Session starts - no state yet**

**User**: "Create opportunity for Acme Capital LP, a Swiss corporate investor"

**System generates DSL (State Version 1)**:
```lisp
(investor.start-opportunity
  @attr{a1b2c3d4-...}  ; investor_id (generated UUID)
  @attr{e5f6a7b8-...}  ; investor.legal_name = "Acme Capital LP"
  @attr{c9d0e1f2-...}  ; investor.type = "CORPORATE"
  @attr{a3b4c5d6-...}  ; investor.domicile = "CH"
)
```

**Dictionary lookups**:
- `a1b2c3d4-...` → `investor_id` (uuid, primary key)
- `e5f6a7b8-...` → `investor.legal_name` (string, PII, required)
- `c9d0e1f2-...` → `investor.type` (enum: PROPER_PERSON|CORPORATE|TRUST|FOHF)
- `a3b4c5d6-...` → `investor.domicile` (string, ISO-3166-1 alpha-2)

**Chat State**:
```json
{
  "session_id": "uuid-session",
  "built_dsl": "(investor.start-opportunity @attr{a1b2c3d4-...} ...)",
  "context": {
    "investor_id": "a1b2c3d4-...",
    "investor_name": "Acme Capital LP",
    "investor_type": "CORPORATE",
    "current_state": "OPPORTUNITY"
  }
}
```

---

**User**: "Start KYC for this investor"

**System uses context** (investor_id from previous state) and **generates new DSL fragment**:
```lisp
(kyc.begin
  @attr{a1b2c3d4-...}  ; investor_id (from context!)
  @attr{f7g8h9i0-...}  ; kyc.tier = "STANDARD"
)
```

**System accumulates DSL (State Version 2)**:
```lisp
(investor.start-opportunity
  @attr{a1b2c3d4-...}
  @attr{e5f6a7b8-...}
  @attr{c9d0e1f2-...}
  @attr{a3b4c5d6-...}
)

(kyc.begin
  @attr{a1b2c3d4-...}  ; SAME investor_id - referential integrity!
  @attr{f7g8h9i0-...}
)
```

**Dictionary lookups**:
- `f7g8h9i0-...` → `kyc.tier` (enum: SIMPLIFIED|STANDARD|ENHANCED)

**Chat State** (updated):
```json
{
  "session_id": "uuid-session",
  "built_dsl": "(investor.start-opportunity ...) (kyc.begin ...)",
  "context": {
    "investor_id": "a1b2c3d4-...",
    "investor_name": "Acme Capital LP",
    "investor_type": "CORPORATE",
    "current_state": "KYC_PENDING"  ← State transition!
  }
}
```

---

**User**: "Collect their certificate of incorporation"

**System generates DSL (State Version 3)**:
```lisp
(investor.start-opportunity
  @attr{a1b2c3d4-...}
  @attr{e5f6a7b8-...}
  @attr{c9d0e1f2-...}
  @attr{a3b4c5d6-...}
)

(kyc.begin
  @attr{a1b2c3d4-...}
  @attr{f7g8h9i0-...}
)

(kyc.collect-doc
  @attr{a1b2c3d4-...}  ; SAME investor_id
  @attr{j1k2l3m4-...}  ; document.type = "certificate of incorporation"
  @attr{e5f6a7b8-...}  ; document.subject = "Acme Capital LP" (reused!)
)
```

**Key observations**:
1. **State accumulates** - each operation appends to DSL
2. **AttributeIDs provide referential integrity** - same investor_id used across operations
3. **AttributeIDs can be reused** - `e5f6a7b8` used for both investor name and document subject
4. **Dictionary provides semantics** - systems know what each attribute means
5. **Complete audit trail** - entire conversation visible in DSL
6. **State transitions tracked** - OPPORTUNITY → KYC_PENDING
7. **No placeholders** - all UUIDs resolved from context

This is how **DSL-as-State + AttributeID-as-Type** enables stateful, semantic, auditable workflows!

---

## 🎯 Why This Architecture Works

The combination of **DSL-as-State** + **AttributeID-as-Type** solves fundamental problems in financial onboarding:

### Traditional Problems Solved

| Traditional Approach | DSL-as-State Solution |
|---------------------|----------------------|
| **State scattered across tables** | State = one DSL document |
| **Audit trail requires event logging** | DSL IS the audit trail |
| **Hard to reconstruct past state** | Parse any DSL version |
| **Type info in code** | Type info in dictionary (metadata) |
| **Validation in multiple places** | Validation via approved verbs |
| **Privacy handled separately** | Privacy in attribute definition |
| **Data contracts break easily** | AttributeID is stable contract |
| **Workflow coordination complex** | DSL is universal language |

### Key Benefits Realized

1. **Regulatory Compliance**: Complete, immutable audit trail required by financial regulators
2. **AI Integration**: Structured DSL enables AI agents to participate in workflows safely (with verb validation)
3. **Cross-System Coordination**: Multiple systems consume same DSL document
4. **Human Readability**: Business users can review and understand DSL
5. **Machine Executability**: Systems can parse and execute DSL operations
6. **Evolvability**: Dictionary can evolve without breaking existing DSL
7. **Data Governance**: Privacy, classification, validation all metadata-driven
8. **Time Travel**: Access any historical state by version number

### This Is Not Just Another DSL

This is a **state representation language** where:
- The language IS the state
- Types ARE semantic identifiers
- Execution IS state transitions
- History IS version accumulation
- Compliance IS inherent in design

**This is the architectural foundation that makes sophisticated financial onboarding workflows tractable, auditable, and AI-enabled.**

## Core Architecture Patterns

**DSL-as-State (Primary Pattern)**: The accumulated DSL document IS the state. Each operation appends to the DSL, creating a new version. State is reconstructed by parsing the DSL. See "Core Architectural Pattern" section above.

**Event Sourcing Pattern**: Uses immutable versioning where each state change creates a new database record rather than updating existing ones. This provides complete audit trails and ability to reconstruct any historical state. The DSL itself acts as the event log.

**State Machine Progression**:
1. **CREATE** (`create` command) - Initial case creation with CBU ID
2. **ADD_PRODUCTS** (`add-products` command) - Append products to existing case
3. **DISCOVER_KYC** (`discover-kyc` command) - AI-assisted KYC discovery using Gemini
4. **DISCOVER_SERVICES** (`discover-services` command) - Service discovery and planning
5. **DISCOVER_RESOURCES** (`discover-resources` command) - Resource discovery and planning

## Development Commands

**Build** (uses experimental `greenteagc` GC for 60% better pause times):
```bash
make build-greenteagc    # Preferred build method
./build.sh              # Alternative script-based build
make test               # Run all tests
make test-coverage      # Generate coverage report
make lint               # Run golangci-lint with 20+ linters
```

**Database Setup**:
```bash
export DB_CONN_STRING="postgres://localhost:5432/postgres?sslmode=disable"
make init-db            # Initialize schema and tables
./dsl-poc seed-catalog  # Populate with mock product/service data
```

**Development Workflow**:
```bash
./dsl-poc create --cbu="CBU-1234"
./dsl-poc add-products --cbu="CBU-1234" --products="CUSTODY,FUND_ACCOUNTING"
./dsl-poc discover-kyc --cbu="CBU-1234"  # Requires GEMINI_API_KEY
./dsl-poc discover-services --cbu="CBU-1234"  # Service discovery and planning
./dsl-poc discover-resources --cbu="CBU-1234"  # Resource discovery and planning
./dsl-poc history --cbu="CBU-1234"       # View complete DSL evolution
```

## Key Architecture Components

**Database Schema** (`sql/init.sql`):
- `dsl_ob` - Immutable versioned DSL records (event sourcing core)
- `products`, `services`, `prod_resources` - Catalog tables
- `attributes`, `dictionaries` - Data classification with privacy flags
- Uses `"ob-poc"` schema with UUID primary keys (legacy name normalized)

**Package Structure**:
- `internal/cli/` - Command implementations for state machine operations
- `internal/store/` - PostgreSQL operations with comprehensive error handling
- `internal/dsl/` - S-expression builders and parsers
- `internal/agent/` - Gemini AI integration for KYC discovery
- `internal/mocks/` - Test data generators

**AI Integration** (`internal/agent/agent.go`):
- Uses Google Gemini 2.5 Flash for KYC requirement discovery
- Structured JSON responses parsed into DSL
- Graceful fallback when API key not provided
- Safety settings configured to avoid blocking

## DSL Format

S-expressions with nested structure representing onboarding progression:

```lisp
(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")

(kyc.start
  (documents
    (document "CertificateOfIncorporation")
  )
  (jurisdictions
    (jurisdiction "LU")
  )
)
```

## Testing Strategy

- Comprehensive unit tests across all packages
- SQL operations mocked using `go-sqlmock`
- DSL generation and parsing tested with realistic scenarios
- CLI command logic tested with various input combinations
- Run single test: `go test -v ./internal/cli -run TestHistoryCommand`
- Run all tests: `make test`
- Generate coverage report: `make test-coverage`

## Performance Notes

**greenteagc Benefits**: 60% reduction in GC pause times, ~4% better throughput, more predictable latency for concurrent workloads (requires Go 1.21+).

**Database Optimizations**: Composite indexes on `(cbu_id, created_at DESC)` for fast latest lookups, soft deletes preserve data integrity, foreign key constraints with appropriate cascades.

## Code Quality

**Linting and Formatting**:
```bash
make lint               # Run golangci-lint with 20+ linters
make fmt                # Format code with gofmt
make vet                # Run go vet
make check              # Run fmt, vet, and lint (pre-commit check)
```

## CI/CD

GitHub Actions pipeline runs on Ubuntu with Go version from `go.mod`, caches modules and build artifacts, executes lint/build/test phases with 5-minute timeout.

## Recent Enhancements

### DSL Verb Validation (Completed)
**Main Onboarding POC Agent**: Added validation to prevent AI from generating unapproved DSL verbs:
- `internal/agent/dsl_agent.go` - Added `validateDSLVerbs()` function with 68 approved verbs
- Validation runs after AI generation in `CallDSLTransformationAgent`
- Returns error if unapproved verbs detected (e.g., "invalid verb: xyz.unknown")
- System prompt explicitly lists all approved verbs from vocabulary
- Pattern: `(verb.action` extracted via regex, validated against approved vocabulary map

**Hedge Fund Module**: Already had validation in place:
- `hedge-fund-investor-source/hf-agent/hf_dsl_agent.go` - Validates 17 hedge fund investorDeferred to Next Session)

**DSL CRUD Operations Enhancement**: The onboarding DSL is the key artifact of this POC. Current implementation has temporary workarounds that need to be completed:

1. **Update DSL functions to use DataStore interface**: Functions like `PopulateAttributeValues` currently expect concrete store types
2. **Complete attribute resolution workflow**: The `populate-attributes` and `get-attribute-values` commands need full DataStore integration
3. **Implement missing DataStore methods**: Some operations like `GetAttributesForDictionaryGroup` are commented out
4. **Enhance mock data error handling**: Improve graceful handling for missing mock data files
5. **Complete integration test refactoring**: Update skipped tests to work with DataStore interface injection

These tasks are critical for the full onboarding workflow but were deferred to focus on completing the DataStore interface abstraction successfully.
