# Build & Development Commands

All automation via xtask (type-safe Rust, cross-platform).

## Quick Reference
```bash
cd rust/

# Development workflow
cargo x pre-commit          # Format + clippy + unit tests (fast)
cargo x check               # Compile + clippy + tests
cargo x check --db          # Include database integration tests

# Individual tasks
cargo x fmt                 # Format code
cargo x clippy              # Run clippy on all feature combinations
cargo x clippy --fix        # Auto-fix clippy warnings
cargo x test                # Run all tests
cargo x test --lib          # Unit tests only (faster)

# Build
cargo x build               # Build all binaries (debug)
cargo x build --release     # Release builds
cargo x wasm                # Build WASM components
cargo x wasm --release      # Release WASM

# Deploy (recommended for UI development)
cargo x deploy              # Full deploy: WASM + server + start
cargo x deploy --release    # Release builds
cargo x deploy --skip-wasm  # Skip WASM if only Rust changed

# Utilities
cargo x schema-export       # Export DB schema to schema_export.sql
cargo x ts-bindings         # Generate TypeScript from ob-poc-types
cargo x dsl-tests           # Run DSL test scenarios
cargo x serve               # Start web server (port 3000)

# CI
cargo x ci                  # Full pipeline: fmt, clippy, test, build
```

## Before Committing
```bash
cargo x pre-commit
```

## Full Local CI
```bash
cargo x ci
```
