# Makefile for Go web server

LINTER = golangci-lint run

.PHONY: lint build test clean

# Target to lint the code with all experimental features enabled
lint:
	@echo "--- Running linter with Go experiments enabled ---"
	GOEXPERIMENT=greenteagc,jsonv2 $(LINTER)

# Build the web server
build:
	@echo "--- Building web server ---"
	go build -o bin/webserver ./cmd/web

# Build the harness
harness:
	@echo "--- Building harness ---"
	go build -o bin/harness ./cmd/harness

# Run tests
test:
	@echo "--- Running tests ---"
	go test ./...

# Clean build artifacts
clean:
	@echo "--- Cleaning ---"
	rm -rf bin/

# Run all checks
check: lint test
	@echo "--- All checks passed ---"
