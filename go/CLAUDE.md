# Go Test Harness

Test harness for the OB-POC DSL system. Calls Rust `dsl_api` server via HTTP.

## Architecture

```
┌─────────────────────┐     ┌─────────────────────┐
│  Go Harness         │────▶│  Rust dsl_api       │
│  :8181              │     │  :3001              │
│                     │     │                     │
│  - Web UI           │     │  - /validate        │
│  - Test suites      │     │  - /execute         │
│  - Assertions       │     │  - /query/cbus      │
│  - Cleanup          │     │  - /cleanup/cbu/:id │
└─────────────────────┘     └─────────────────────┘
```

## Directory Structure

```
go/
├── cmd/
│   ├── harness/main.go    # CLI test runner
│   └── web/
│       ├── main.go        # Web server (:8181)
│       ├── templates/     # HTML templates
│       └── static/        # Static assets
├── internal/
│   ├── rustclient/
│   │   ├── client.go      # HTTP client for dsl_api
│   │   ├── api_types.go   # API request/response types
│   │   └── types.go       # Graph visualization types
│   └── harness/
│       └── harness.go     # Test suite/case framework
├── bin/                   # Built binaries (gitignored)
├── go.mod
└── CLAUDE.md              # This file
```

## Commands

```bash
cd go/

# Build
go build ./...
go build -o bin/harness ./cmd/harness
go build -o bin/web ./cmd/web

# Run web UI
./bin/web                              # Standalone mode
./bin/web -rust-url http://localhost:3001  # Connected to dsl_api

# Run CLI harness
./bin/harness -url http://localhost:3001
```

## API Client (rustclient)

The `rustclient` package provides typed HTTP access to `dsl_api`:

```go
client := rustclient.NewClient("http://localhost:3001")

// Health check
health, _ := client.Health(ctx)

// Validate DSL
result, _ := client.ValidateDSL(ctx, `(cbu.ensure :name "Test" :jurisdiction "LU")`)

// Execute DSL
resp, _ := client.ExecuteDSL(ctx, dsl)
for name, id := range resp.Bindings {
    fmt.Printf("Created %s: %s\n", name, id)
}

// Query data
cbus, _ := client.ListCBUs(ctx)
cbu, _ := client.GetCBU(ctx, cbuID)
kycCase, _ := client.GetKYCCase(ctx, caseID)

// Cleanup
client.CleanupCBU(ctx, cbuID)
```

## Test Harness (harness)

The `harness` package provides a test suite framework:

```go
suite := harness.Suite{
    Name: "Onboarding Tests",
    Cases: []harness.Case{
        {
            Name: "Create CBU",
            DSL:  `(cbu.ensure :name "Test" :jurisdiction "LU" :as @cbu)`,
            Expect: harness.Expectation{
                Success:     true,
                EntityCount: intPtr(1),
            },
        },
        {
            Name: "Invalid DSL",
            DSL:  `(invalid.verb :foo "bar")`,
            Expect: harness.Expectation{
                Success: false,
            },
        },
    },
}

runner := harness.NewRunner("http://localhost:3001")
result, _ := runner.Run(ctx, suite)

// Cleanup created entities
runner.Cleanup(ctx, result.CreatedIDs)
```

## Key Types

### ExecuteResponse
```go
type ExecuteResponse struct {
    Success  bool
    Results  []ExecuteResultItem
    Bindings map[string]uuid.UUID  // @symbol -> UUID
    Errors   []string
}
```

### CbuDetail
```go
type CbuDetail struct {
    CbuID        uuid.UUID
    Name         string
    Jurisdiction *string
    ClientType   *string
    Entities     []EntityRole  // Entities with roles
}
```

### KycCaseDetail
```go
type KycCaseDetail struct {
    CaseID      uuid.UUID
    CbuID       uuid.UUID
    Status      string
    Workstreams []WorkstreamDetail
    RedFlags    []RedFlagDetail
}
```

## Testing Pattern

1. **Execute DSL** to create test data
2. **Query** via `/query/cbus/:id` to verify state
3. **Assert** expected values
4. **Cleanup** via `/cleanup/cbu/:id`

```go
// 1. Execute
resp, _ := client.ExecuteDSL(ctx, `
    (cbu.ensure :name "Test Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
    (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
    (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
`)

// 2. Query
cbuID := resp.Bindings["fund"]
cbu, _ := client.GetCBU(ctx, cbuID)

// 3. Assert
assert.Equal(t, "Test Fund", cbu.Name)
assert.Equal(t, 1, len(cbu.Entities))
assert.Equal(t, "DIRECTOR", cbu.Entities[0].Role)

// 4. Cleanup
client.CleanupCBU(ctx, cbuID)
```

## Dependencies

- `github.com/google/uuid` - UUID handling
