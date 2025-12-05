# Build stage - compiles Rust binary
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first (for better caching)
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --features server --bin agentic_server 2>/dev/null || true

# Copy actual source
COPY src/ src/
COPY config/ config/

# Build the real binary
RUN touch src/main.rs && \
    cargo build --release --features server --bin agentic_server

# Runtime stage - minimal image with just the binary
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary
COPY --from=builder /app/target/release/agentic_server /app/agentic_server

# Copy config files (verb definitions)
COPY --from=builder /app/config/ /app/config/

# Environment variables
ENV RUST_LOG=ob_poc=info
ENV DSL_CONFIG_DIR=/app/config

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/agent/health || exit 1

# Run the server
CMD ["/app/agentic_server"]
