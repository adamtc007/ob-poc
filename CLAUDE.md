# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**OB-POC** is a multi-language Ultimate Beneficial Ownership (UBO) and comprehensive onboarding system implementing a declarative DSL approach. The project demonstrates **DSL-as-State** architecture where accumulated DSL documents serve as both state representation and audit trail.

## Core Architecture: DSL-as-State + AttributeID-as-Type

### DSL-as-State Pattern
The fundamental pattern: **The accumulated DSL document IS the state itself**.

- **State = Accumulated DSL Document**: Each onboarding case's current state is represented by its complete, accumulated DSL document
- **Immutable Event Sourcing**: Each operation appends to the DSL, creating new immutable versions
- **Executable Documentation**: DSL serves as human-readable documentation, machine-parseable data, audit trail, and workflow definition

### AttributeID-as-Type Pattern
Variables in DSL are typed by AttributeID (UUID) referencing a universal dictionary, not primitive types.

Example structure:
```lisp
(verb @attr{uuid-001} @attr{uuid-002} ...)
```

Where each UUID references the dictionary table containing:
- Data type and validation rules
- Privacy classification (PII, PCI, PHI)
- Source/sink metadata
- Business domain context

## Multi-Language Implementation

### Rust Implementation (`/rust/`)
**Primary DSL engine and compiler** - Production system for orchestration and persistence.

**Key Commands:**
```bash
# Development workflow
cd rust/
./dev-check.sh              # Quick compilation, clippy, and test check
./dev-commit.sh "message"    # Full development workflow with commit
bacon                        # Auto-rebuild on save (default: cargo check)

# CLI operations
cargo build
cargo run --bin cli examples/zenith_capital_ubo.dsl
cargo test                   # Run all tests
cargo clippy                 # Linting
```

**Key Components:**
- **DSL Engine**: NOM-based parsing with EDN-style syntax
- **Database Integration**: PostgreSQL with soft schema JSON AST storage
- **Graph Modeling**: Property graphs for ownership structures
- **Domain Support**: KYC, onboarding, account opening workflows

### Go Implementation (`/go/`)
**Demo and mock system** - Retained for demonstrations; production orchestration moved to Rust.

**Key Commands:**
```bash
# Build with experimental GC (60% better pause times)
make build-greenteagc

# Database operations (deprecated; use Rust backend)
export DB_CONN_STRING="postgres://user:password@localhost:5432/db?sslmode=disable"
make init-db

# Development workflow
make check                   # fmt, vet, lint
make test-coverage          # Tests with coverage report
make lint                   # golangci-lint with 20+ linters

# DSL operations (demo only)
./dsl-poc create --cbu="CBU-1234"
./dsl-poc add-products --cbu="CBU-1234" --products="CUSTODY,FUND_ACCOUNTING"
./dsl-poc history --cbu="CBU-1234"
```

## Database Schema

**Canonical PostgreSQL Schema**: `"ob-poc"` (legacy references to `"dsl-ob-poc"` exist only in migration scripts)

**Setup:**
```bash
# Initialize schema
psql -d your_database -f sql/00_init_schema.sql
psql -d your_database -f sql/03_seed_dictionary_attributes.sql
```

**Key Tables:**
- **Grammar Management**: EBNF grammars with versioning and composition
- **Domain Organization**: Hierarchical business domain structure (KYC, Onboarding, Account Opening)
- **Shared Vocabulary**: Common attributes, verbs, and data types in dictionary table
- **DSL Compilation**: AST storage and execution tracking
- **Audit Trail**: Complete change tracking and compliance logging

## DSL Syntax Examples

### KYC UBO Discovery
```clojure
(define-kyc-investigation "zenith-capital-ubo-discovery"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :ubo-threshold 25.0

  (declare-entity
    :node-id "company-zenith-spv-001"
    :label Company
    :properties {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456"
      :jurisdiction "KY"
    })

  (validate customer.email_primary "john@example.com")
  (collect kyc.risk_rating :from "risk-engine")
  (check compliance.fatca_status :equals "NON_US")

  (create-edge
    :from "alpha-holdings-sg"
    :to "company-zenith-spv-001"
    :type HAS_OWNERSHIP
    :properties {
      :percent 45.0
      :share-class "Class A Ordinary"
    }
    :evidenced-by ["doc-cayman-registry-001"]))
```

### Client Onboarding Progression
```lisp
(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU"))

(products.add "CUSTODY" "FUND_ACCOUNTING")

(kyc.start
  (documents (document "CertificateOfIncorporation"))
  (jurisdictions (jurisdiction "LU")))

(services.plan
  (service "Settlement" (sla "T+1")))

(resources.plan
  (resource "CustodyAccount" (owner "CustodyTech")))
```

## Development Workflows

### Rust Development
```bash
cd rust/

# Quick development check
./dev-check.sh

# Full development workflow with commit
./dev-commit.sh "Your commit message"

# Auto-rebuild on save
bacon

# Specific operations
cargo test --lib                    # Unit tests only
cargo test integration             # Integration tests
cargo run --example parse_zenith   # Run example
cargo doc --open                   # Generate and open docs
```

### Go Development (Demo/Mock Mode)
```bash
cd go/

# Code quality pipeline
make check              # fmt + vet + lint
make test-coverage      # Tests with HTML coverage report

# Build options
make build-greenteagc   # Experimental GC (recommended)
make build             # Standard build

# Testing
go test -v ./internal/cli -run TestHistoryCommand  # Single test
make test                                          # All tests
```

## Testing Strategy

### Rust Testing
- **Unit Tests**: Core parsing and DSL logic
- **Integration Tests**: Database operations and end-to-end workflows
- **Example Testing**: Validate DSL parsing with real-world examples
- **Property Testing**: Grammar validation and AST generation

### Go Testing (Demo System)
- **Mock Testing**: Uses go-sqlmock for database operations
- **CLI Testing**: Command implementations with various input combinations
- **DSL Validation**: S-expression generation and parsing scenarios

## Key Features and Domains

### Multi-Domain Support
- **Back Office**: KYC/AML, Contracting, Invoicing
- **Front Office**: Account Opening, Trade & Instruction Capture, Data Delivery
- **Data Management**: Data Operations and analytics

### Business Domain Examples
- **Hedge Fund Investor Onboarding**: Subscription processes, KYC workflows, regulatory compliance
- **UCITS Fund Setup**: Multi-jurisdiction fund establishment, custody arrangements
- **Corporate Banking**: Enhanced KYC, cash management, trade finance
- **Ultimate Beneficial Ownership**: Entity relationship modeling, compliance calculations

### Compliance & Audit Features
- **Immutable Audit Trail**: Every decision captured in DSL versions
- **Document-based Evidence**: Complete evidence tracking for regulatory requirements
- **Privacy by Design**: Data governance embedded in AttributeID type system
- **Regulatory Reporting**: Support for CRS, FATCA, and other compliance frameworks

## AI Integration

### Gemini AI Integration (Go Demo)
```bash
# Enable AI-assisted KYC discovery
export GEMINI_API_KEY="your-gemini-api-key"
./dsl-poc discover-kyc --cbu="CBU-1234"
```

### AI Safety Features
- **Verb Validation**: Only approved DSL verbs allowed (prevents AI hallucination)
- **Structured Vocabulary**: 70+ approved verbs in main system, 17 in hedge fund module
- **Context Validation**: UUID resolution and referential integrity checking

## Architecture Philosophy

### Keywords as First-Class Citizens
Following Rich Hickey's EDN philosophy:
- **Self-Documenting**: `:customer.first_name` is immediately recognizable
- **Functional**: Keywords can look themselves up: `(:name entity)`
- **Namespace Safe**: `:compliance/ubo-threshold` prevents naming conflicts
- **Data as Code**: Same reader parses DSL and data structures

### Soft Schema Design
- **AST Storage**: JSON AST storage for flexibility and language-agnostic interchange
- **Schema Evolution**: No migrations required for dictionary changes
- **Type Evolution**: AttributeID references provide stable contracts

### Enterprise Benefits
- **Configuration Over Code**: Business rule changes via dictionary updates, not code changes
- **Cross-System Coordination**: Universal DSL language for all enterprise systems
- **Time Travel**: Complete historical state reconstruction from any DSL version
- **Regulatory Compliance**: Built-in audit trails and evidence tracking

## Performance Notes

### Rust Performance
- **NOM Parsing**: High-performance parser combinator library for DSL processing
- **PostgreSQL Integration**: Optimized JSON AST storage with proper indexing
- **Async Processing**: Tokio-based async runtime for concurrent operations

### Go Performance (Demo)
- **greenteagc GC**: 60% reduction in GC pause times, 4% better throughput
- **Database Optimizations**: Composite indexes on `(cbu_id, created_at DESC)`
- **Connection Pooling**: Efficient database connection management

## Migration and Compatibility

### Database Migration
- **Legacy Support**: Migration scripts handle `"dsl-ob-poc"` to `"ob-poc"` schema normalization
- **Data Preservation**: All existing DSL versions preserved during schema updates
- **Backward Compatibility**: Existing DSL documents remain valid after dictionary evolution

### Cross-Language Compatibility
- **Shared Schema**: Both Rust and Go implementations use the same PostgreSQL schema
- **Compatible DSL**: S-expression format consistent across implementations
- **API Consistency**: REST endpoints and data structures aligned between languages

## Known Limitations and Future Work

### Current Limitations
- **Go Direct DB Access**: Deprecated in favor of Rust orchestration layer
- **Mock Data Dependencies**: Some Go operations require specific mock data files
- **CRUD Operations**: Some DSL operations need complete DataStore interface integration

### Planned Enhancements
- **Web-based Visualization**: egui/WASM frontend for interactive AST visualization
- **Enhanced Domain Support**: Expanded business domain vocabularies
- **Real-time Collaboration**: Multi-user editing capabilities
- **Extended AI Integration**: More sophisticated LLM-assisted workflow generation

## Debugging and Troubleshooting

### Common Issues
1. **Database Connection**: Ensure `DB_CONN_STRING` is properly set
2. **Schema Mismatch**: Use latest migration scripts from `/sql/`
3. **Missing Dependencies**: Run `cargo build` or `make install-deps`
4. **Test Failures**: Check mock data availability and database state

### Debugging Tools
```bash
# Rust debugging
RUST_LOG=debug cargo run --bin cli
cargo test -- --nocapture              # See test output

# Go debugging
go test -v ./... -run FailingTest      # Verbose test output
./dsl-poc --help                       # CLI help and commands
```

## Contributing Guidelines

1. **Choose Implementation**: Use Rust for production features, Go for demos/mocks
2. **Follow Patterns**: Implement DSL-as-State and AttributeID-as-Type patterns
3. **Update Documentation**: Modify `/sql/` schemas and grammar definitions as needed
4. **Test Thoroughly**: Add tests for new DSL verbs, domains, and integrations
5. **Maintain Compatibility**: Ensure changes work across both language implementations

## License

MIT License - Internal POC development