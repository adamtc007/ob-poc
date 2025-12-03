# Go Test Harness Implementation Plan

## Overview

Add a Go section to ob-poc for test harnesses that call the Rust DSL pipeline via HTTP API. The Go layer provides:
- Test harness library for automated DSL testing
- Simple web UI to run tests and view results
- HTTP client to call Rust API endpoints

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Go Test Harness Web UI                        │
│                    (localhost:8080)                              │
│  - Run test suites                                               │
│  - View results with pass/fail status                           │
│  - Browse test scenarios                                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Go Test Harness Library                       │
│  go/internal/harness/                                           │
│  - TestSuite, TestCase, TestResult types                        │
│  - DSL execution via Rust API                                   │
│  - Assertion helpers                                             │
│  - Result collection and reporting                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Go Rust API Client                            │
│  go/internal/rustclient/                                        │
│  - HTTP client for Rust API (localhost:3000)                    │
│  - Type-safe request/response structs                           │
│  - DSL validate, execute, generate                               │
│  - CBU, Entity, KYC operations                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Rust Agentic Server                           │
│                    (localhost:3000)                              │
│  - POST /api/agent/validate                                      │
│  - POST /api/session/:id/execute                                │
│  - GET  /api/cbu, /api/cbu/:id/graph                            │
└─────────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
ob-poc/
├── go/
│   ├── go.mod                      # Go module: github.com/ob-poc/harness
│   ├── go.sum
│   ├── cmd/
│   │   ├── harness/                # CLI for running tests
│   │   │   └── main.go
│   │   └── server/                 # Web UI server
│   │       └── main.go
│   ├── internal/
│   │   ├── rustclient/             # Rust API HTTP client
│   │   │   ├── client.go           # Client struct, base HTTP
│   │   │   ├── dsl.go              # DSL validate/execute
│   │   │   ├── cbu.go              # CBU operations
│   │   │   └── types.go            # Request/response types
│   │   ├── harness/                # Test harness library
│   │   │   ├── suite.go            # TestSuite, Run()
│   │   │   ├── case.go             # TestCase definition
│   │   │   ├── result.go           # TestResult, assertions
│   │   │   ├── loader.go           # Load tests from YAML/JSON
│   │   │   └── reporter.go         # Output formatting
│   │   └── web/                    # Web UI handlers
│   │       ├── server.go           # HTTP server setup
│   │       ├── handlers.go         # API handlers
│   │       └── templates/          # HTML templates
│   │           ├── layout.html
│   │           ├── index.html
│   │           └── results.html
│   └── tests/                      # Test scenario definitions
│       ├── scenarios/
│       │   ├── cbu_onboarding.yaml
│       │   ├── kyc_flow.yaml
│       │   └── custody_setup.yaml
│       └── fixtures/               # Test data
└── rust/                           # Existing Rust code (unchanged)
```

## Key Components

### 1. Rust API Client (go/internal/rustclient/)

```go
// client.go
type Client struct {
    BaseURL    string
    HTTPClient *http.Client
}

func New(baseURL string) *Client

// dsl.go
func (c *Client) ValidateDSL(dsl string) (*ValidationResult, error)
func (c *Client) ExecuteDSL(sessionID, dsl string) (*ExecutionResult, error)
func (c *Client) CreateSession() (*Session, error)

// cbu.go
func (c *Client) ListCBUs() ([]CBUSummary, error)
func (c *Client) GetCBUGraph(cbuID uuid.UUID) (*CBUGraph, error)
```

### 2. Test Harness Library (go/internal/harness/)

```go
// suite.go
type TestSuite struct {
    Name        string
    Description string
    Setup       []string      // DSL to run before tests
    Teardown    []string      // DSL to run after tests
    Cases       []TestCase
}

func (s *TestSuite) Run(client *rustclient.Client) *SuiteResult

// case.go
type TestCase struct {
    Name        string
    Description string
    DSL         string
    Expect      Expectation
}

type Expectation struct {
    Success     bool
    Bindings    map[string]interface{}  // Expected @symbol values
    ErrorMatch  string                   // Regex for expected error
}
```

### 3. Web UI Server (go/cmd/server/)

Simple web server with endpoints:
- GET /           - Dashboard with test suites
- GET /suites     - List available test suites
- POST /suites/:name/run - Run a test suite
- GET /results/:id - View test results
- GET /api/health - Health check

### 4. Test Scenario Format (YAML)

```yaml
# go/tests/scenarios/cbu_onboarding.yaml
name: CBU Onboarding Flow
description: Test complete CBU onboarding with entities and KYC

setup:
  - |
    ;; Clean test data if exists
    
cases:
  - name: Create CBU
    dsl: |
      (cbu.ensure :name "Test Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)
    expect:
      success: true
      bindings:
        fund: "uuid"  # Expect UUID binding

  - name: Add Director
    dsl: |
      (entity.create-proper-person :first-name "John" :last-name "Smith" :as @director)
      (cbu.assign-role :cbu-id @fund :entity-id @director :role "DIRECTOR")
    expect:
      success: true

  - name: Invalid Verb Should Fail
    dsl: |
      (invalid.verb :foo "bar")
    expect:
      success: false
      error_match: "unknown verb"
```

## Implementation Steps

### Phase 1: Foundation
1. Create go/ directory structure
2. Initialize Go module
3. Implement rustclient package with basic HTTP client
4. Add DSL validate/execute methods

### Phase 2: Test Harness
5. Implement harness package with TestSuite/TestCase
6. Add YAML test loader
7. Create CLI runner (cmd/harness)
8. Add sample test scenarios

### Phase 3: Web UI
9. Implement web server (cmd/server)
10. Create HTML templates
11. Add API endpoints for running tests
12. Display results with pass/fail styling

## Commands

```bash
# Run tests via CLI
cd go && go run ./cmd/harness -suite cbu_onboarding

# Start web UI
cd go && go run ./cmd/server
# Open http://localhost:8080

# Run all tests
cd go && go test ./...
```

## Dependencies

Minimal Go dependencies:
- gopkg.in/yaml.v3 - YAML parsing for test scenarios
- github.com/google/uuid - UUID handling
- Standard library for HTTP client/server

## Notes

- Rust server must be running on localhost:3000
- Go calls Rust via HTTP, no FFI/CGO
- Test scenarios are declarative YAML
- Web UI is server-rendered HTML (no JS framework)
