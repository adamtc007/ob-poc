# Makefile for entity-gateway

.PHONY: proto build test clean run

# Default target
all: proto build

# Generate protobuf code (required before build)
proto:
	@echo "--- Generating protobuf code ---"
	@mkdir -p src/proto
	protoc --proto_path=proto \
		--rust_out=src/proto \
		--grpc_out=src/proto \
		--plugin=protoc-gen-grpc=`which grpc_rust_plugin` \
		proto/ob/gateway/v1/entity_gateway.proto || \
	cargo build --message-format=short 2>&1 | head -5  # fallback: let tonic-build handle it

# Build the gateway
build:
	@echo "--- Building entity-gateway ---"
	cargo build

# Build release
release:
	@echo "--- Building entity-gateway (release) ---"
	cargo build --release

# Run tests
test:
	@echo "--- Running tests ---"
	cargo test

# Clean build artifacts
clean:
	@echo "--- Cleaning ---"
	cargo clean
	rm -rf src/proto/*.rs

# Run the gateway server
run:
	@echo "--- Running entity-gateway ---"
	cargo run

# Clippy lint
lint:
	@echo "--- Running clippy ---"
	cargo clippy -- -D warnings

# Format code
fmt:
	@echo "--- Formatting code ---"
	cargo fmt

# Check formatting
fmt-check:
	@echo "--- Checking formatting ---"
	cargo fmt -- --check
