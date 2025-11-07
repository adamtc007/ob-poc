# DSL Onboarding POC

**Domain-Specific Language for Client Onboarding** - A Go-based proof-of-concept implementing an immutable, versioned state machine for client onboarding with S-expression DSL output.

## 🚀 Quick Start

### Prerequisites
- **Go 1.21+** (requires `GOEXPERIMENT=greenteagc` support)
- **PostgreSQL** (running and accessible)

### Setup & Run
```bash
# 1. Set database connection
export DB_CONN_STRING="postgres://user:password@localhost:5432/your_db?sslmode=disable"

# 2. Build with optimized GC (60% better pause times)
make build-greenteagc

# 3. Initialize database and seed catalog
./dsl-poc init-db
./dsl-poc seed-catalog

# 4. Run the onboarding workflow
./dsl-poc create --cbu="CBU-1234"
./dsl-poc add-products --cbu="CBU-1234" --products="CUSTODY,FUND_ACCOUNTING"
./dsl-poc discover-services --cbu="CBU-1234"
./dsl-poc discover-resources --cbu="CBU-1234"
./dsl-poc populate-attributes --cbu="CBU-1234"
./dsl-poc get-attribute-values --cbu="CBU-1234"

# 5. View complete DSL evolution
./dsl-poc history --cbu="CBU-1234"
```

## 🏗️ Architecture

### Core Concepts
- **Event Sourcing**: Immutable versioning where each state change creates a new database record
- **State Machine**: 7-stage progression from case creation to attribute value binding
- **S-Expression DSL**: Lisp-like syntax for structured onboarding specifications
- **Entity Relationships**: CBUs (Client Business Units) containing entities with defined roles

### State Machine Progression
1. **CREATE** - Initial case creation with CBU ID
2. **ADD_PRODUCTS** - Append products to existing case
3. **DISCOVER_KYC** - AI-assisted KYC discovery using Gemini
4. **DISCOVER_SERVICES** - Service discovery and planning
5. **DISCOVER_RESOURCES** - Resource discovery and planning
6. **POPULATE_ATTRIBUTES** - Runtime attribute value resolution
7. **GET_ATTRIBUTE_VALUES** - Deterministic value binding and DSL output

### Database Schema
- **Event Sourcing Core**: `dsl_ob` table with versioned DSL records
- **Catalogs**: `products`, `services`, `prod_resources` for service discovery
- **Dictionary**: `dictionary` table with JSONB metadata for attributes
- **Entity Model**: `cbus`, `entities`, `roles`, `cbu_entity_roles` for relationship management
- **Runtime Values**: `attribute_values` with composite keys for versioned data

## 🛠️ Development

### Build Options
```bash
# Recommended: greenteagc (60% better GC pause times)
make build-greenteagc

# Standard build
go build -o dsl-poc .

# Run tests with coverage
make test-coverage

# Lint and format
make lint
make fmt
```

### Entity & CBU Management
```bash
# CBU Management
./dsl-poc cbu-create --name="Aviva Global Fund" --description="UCITS equity fund"
./dsl-poc cbu-list
./dsl-poc cbu-get --id="<cbu-id>"
./dsl-poc cbu-update --id="<cbu-id>" --name="Updated Name"
./dsl-poc cbu-delete --id="<cbu-id>"

# Role Management
./dsl-poc role-create --name="Investment Manager" --description="Manages investment strategies"
./dsl-poc role-list
./dsl-poc role-get --id="<role-id>"
./dsl-poc role-update --id="<role-id>" --name="Updated Role"
./dsl-poc role-delete --id="<role-id>"
```

### AI Integration
```bash
# Optional: Enable AI-assisted KYC discovery
export GEMINI_API_KEY="your-gemini-api-key"
./dsl-poc discover-kyc --cbu="CBU-1234"
```

## 📋 DSL Format

The system generates S-expression DSL representing onboarding progression:

```lisp
(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")

(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
    (service "SettlementService")
  )
)

(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    (var (attr-id "123e4567-e89b-12d3-a456-426614174000"))
  )
)

(values.bind
  (bind (attr-id "123e4567-e89b-12d3-a456-426614174000") (value "CBU-1234"))
)
```

## 🗄️ Entity Relationship Model

**CBU** → **Entity Roles** → **Entities** → **Entity Type Tables**

- **CBUs**: Client Business Units (funds, companies, etc.)
- **Entities**: Limited companies, partnerships, individuals
- **Roles**: Investment manager, asset owner, SiCAV, management company
- **Relationships**: Many-to-many through role assignments

## 🧪 Testing

```bash
# Run all tests
make test

# Run specific test suites
go test -v ./internal/store -run "TestCBU"
go test -v ./internal/dsl -run "TestDSL"

# Generate coverage report
make test-coverage
open coverage.html
```

## 📁 Project Structure

```
├── cmd/                    # CLI entry points
├── internal/
│   ├── agent/             # Gemini AI integration
│   ├── cli/               # Command implementations
│   ├── dsl/               # S-expression builders/parsers
│   ├── store/             # PostgreSQL operations
│   └── dictionary/        # Data classification
├── sql/                   # Database schema
└── CLAUDE.md             # Claude Code guidance
```

## 🚀 Performance

**greenteagc Benefits:**
- 60% reduction in GC pause times
- ~4% better throughput
- More predictable latency for concurrent workloads
- Requires Go 1.21+

## 📚 Additional Documentation

- **[CLAUDE.md](CLAUDE.md)** - Instructions for Claude Code development
- **[SCHEMA_DOCUMENTATION.md](SCHEMA_DOCUMENTATION.md)** - Detailed database schema reference

---

**License**: Internal POC - Not for distribution