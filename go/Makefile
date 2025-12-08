# Makefile for Go web server

LINTER = golangci-lint run
# Enable experimental features: greenteagc (GC) and jsonv2 (new JSON API)
export GOEXPERIMENT=greenteagc,jsonv2

.PHONY: lint build test clean run

# Target to lint the code with all experimental features enabled
lint:
	@echo "--- Running linter with Go experiments enabled ---"
	$(LINTER)

# Build the web server
build:
	@echo "--- Building web server with GOEXPERIMENT=$(GOEXPERIMENT) ---"
	go build -o bin/webserver ./cmd/web

# Build the harness
harness:
	@echo "--- Building harness with GOEXPERIMENT=$(GOEXPERIMENT) ---"
	go build -o bin/harness ./cmd/harness

# Run the web server (for development)
run: build
	@echo "--- Running web server ---"
	./bin/webserver

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
