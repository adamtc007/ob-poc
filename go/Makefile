.PHONY: build build-greenteagc clean install-deps init-db help lint fmt vet deadcode test test-coverage check validate validate-bad

# Default Go variables
GO := go
GOFLAGS := -v
OUTPUT := dsl-poc
GOLANGCI_LINT := golangci-lint

# Default files (override via: make validate FILE=... or make validate-bad BAD_FILE=...)
FILE ?= examples/runbook.sample.json
BAD_FILE ?= examples/bad.runbook.json

help:
	@echo "Available targets:"
	@echo ""
	@echo "Build targets:"
	@echo "  make build              - Build with standard Go GC"
	@echo "  make build-greenteagc   - Build with experimental greenteagc GC (recommended)"
	@echo "  make install-deps       - Install Go dependencies"
	@echo "  make clean              - Remove built binary"
	@echo ""
	@echo "Code quality targets:"
	@echo "  make lint               - Run golangci-lint"
	@echo "  make fmt                - Format code with gofmt"
	@echo "  make vet                - Run go vet"
	@echo "  make deadcode           - Find unreachable code with deadcode tool"
	@echo "  make check              - Run fmt, vet, and lint (pre-commit check)"
	@echo ""
	@echo "Test targets:"
	@echo "  make test               - Run tests"
	@echo "  make test-coverage      - Run tests with coverage report"
	@echo ""
	@echo "Database targets (deprecated in Go; use Rust backend):"
	@echo ""
	@echo "Hedge Fund DSL targets:"
	@echo "  make validate [FILE=examples/runbook.sample.json]      - Validate a good DSL runbook"
	@echo "  make validate-bad [BAD_FILE=examples/bad.runbook.json] - Intentionally validate a bad runbook (non-zero exit)"
	@echo ""
	@echo "Environment variables:"
	@echo "  DB_CONN_STRING - PostgreSQL connection string (required for init-db)"
	@echo ""
	@echo "Examples:"
	@echo "  export DB_CONN_STRING=\"postgres://user:password@localhost:5432/db?sslmode=disable\""
	@echo "  make check              # Check code quality"
	@echo "  make build-greenteagc   # Build with greenteagc"
	@echo "  make init-db            # Initialize database"
	@echo "  make validate           # Validate sample runbook"
	@echo "  make validate FILE=/tmp/runbook.json  # Validate custom file"
	@echo "  make validate-bad       # Test validation with bad runbook"
	@echo "  make -k validate-bad    # Continue after failure to observe the error"
	@echo "  ./dsl-poc create --cbu=\"CBU-1234\""

build: install-deps
	GOCACHE=$(PWD)/.gocache $(GO) build $(GOFLAGS) -o $(OUTPUT) .

build-greenteagc: install-deps
	GOCACHE=$(PWD)/.gocache GOEXPERIMENT=greenteagc $(GO) build $(GOFLAGS) -o $(OUTPUT) .

install-deps:
	GOCACHE=$(PWD)/.gocache $(GO) mod tidy
	GOCACHE=$(PWD)/.gocache $(GO) mod download



clean:
	$(GO) clean
	rm -f $(OUTPUT)
	rm -f coverage.out
	rm -f coverage.html

# Code quality targets
fmt:
	@echo "Running gofmt..."
	@$(GO) fmt ./...

vet:
	@echo "Running go vet..."
	@$(GO) vet ./...

lint:
	@echo "Running golangci-lint..."
	@if command -v $(GOLANGCI_LINT) >/dev/null 2>&1; then \
		GOCACHE=$(PWD)/.gocache GOLANGCI_LINT_CACHE=$(PWD)/.gocache/golangci-lint $(GOLANGCI_LINT) run -v ./...; \
	else \
		echo "golangci-lint is not installed. Install it from https://golangci-lint.run/usage/install/"; \
		exit 1; \
	fi

deadcode:
	@echo "Finding unreachable code with deadcode..."
	@if command -v deadcode >/dev/null 2>&1; then \
		deadcode -test ./...; \
	else \
		echo "deadcode is not installed. Install it with: go install golang.org/x/tools/cmd/deadcode@latest"; \
		exit 1; \
	fi

check: fmt vet lint
	@echo "All checks passed!"

# Test targets
test:
	@echo "Running tests..."
	@GOCACHE=$(PWD)/.gocache $(GO) test -v ./...

test-coverage:
	@echo "Running tests with coverage..."
	@GOCACHE=$(PWD)/.gocache $(GO) test -v -coverprofile=coverage.out ./...
	@GOCACHE=$(PWD)/.gocache $(GO) tool cover -html=coverage.out -o coverage.html
	@echo "Coverage report generated: coverage.html"

# Build and run targets
run: build-greenteagc
	./$(OUTPUT)

# Development workflow
dev: check build-greenteagc
	@echo "Development build complete!"

# Hedge Fund DSL validation
validate:
	@echo "Validating $(FILE) ..."
	@$(GO) run ./cmd/hf-cli dsl-validate -pretty < $(FILE)

validate-bad:
	@echo "Intentionally validating a bad runbook: $(BAD_FILE)"
	@$(GO) run ./cmd/hf-cli dsl-validate < $(BAD_FILE)
