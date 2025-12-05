# Build stage - compiles Go binary
FROM golang:1.22-alpine AS builder

WORKDIR /app

# Install git for go mod download (some deps may need it)
RUN apk add --no-cache git

# Copy go mod files first for better caching
COPY go.mod go.sum* ./
RUN go mod download

# Copy source code
COPY . .

# Build the web server binary
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-w -s" -o /app/web ./cmd/web

# Runtime stage - minimal image with just the binary
FROM alpine:3.19

WORKDIR /app

# Install ca-certificates for HTTPS calls
RUN apk add --no-cache ca-certificates curl

# Copy binary from builder
COPY --from=builder /app/web /app/web

# Expose port
EXPOSE 8181

# Health check
HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:8181/health || exit 1

# Run the server
# Note: rust-url and agent-url are configured at runtime via env vars or flags
CMD ["/app/web", "-addr", ":8181"]
