# UBO DSL gRPC Protocol Buffer Interface

This directory contains the comprehensive gRPC service definitions for the UBO DSL system, providing a type-safe, language-agnostic interface between Go and Rust components.

## Overview

The gRPC interface is designed to replace FFI calls with safe, networked communication between the Go CLI/business logic and the Rust DSL parsing engine. This approach provides:

- **Type Safety**: Protobuf ensures type safety across language boundaries
- **No Unsafe Code**: Go clients use standard gRPC (no unsafe FFI)
- **Language Agnostic**: Any language can use the DSL engine
- **Scalability**: Services can run independently and scale horizontally
- **Network Transparency**: Local or remote deployment with same interface
- **Streaming Support**: Handle large files and batch operations efficiently

## Service Architecture

```
┌─────────────────┐    gRPC     ┌──────────────────┐
│  Go CLI/Server  │ ──────────▶ │  Rust DSL Engine│
│                 │             │                  │
│ • CLI commands  │◀────────────│ • Grammar parsing│
│ • REST API      │             │ • AST generation │
│ • Business logic│             │ • Validation     │
│ • Workflows     │             │ • UBO calculation│
└─────────────────┘             └──────────────────┘
```

## Service Definitions

### Core Services

#### 1. **GrammarService** (`grammar_service.proto`)
- Parse EBNF grammar definitions
- Validate grammar rules and consistency  
- Manage grammar lifecycle (load/unload/activate)
- Export grammars to various formats
- Analyze grammars for complexity and issues

**Key Operations:**
- `ParseGrammar` - Parse EBNF source into grammar AST
- `ValidateGrammar` - Check grammar for errors and ambiguities
- `LoadGrammar` - Load and activate a grammar by name
- `GetGrammarSummary` - Get statistics and dependency info

#### 2. **ParserService** (`parser_service.proto`)
- Parse DSL source code into AST
- Support streaming for large files
- Format and prettify DSL code
- Generate parse trees for debugging

**Key Operations:**
- `ParseDSL` - Parse DSL source into Program AST
- `ParseDSLStream` - Stream parsing for large files
- `ParseAndValidate` - Combined parsing and validation
- `FormatDSL` - Code formatting and prettification

#### 3. **UboService** (`ubo_service.proto`)
- Calculate Ultimate Beneficial Ownership
- Analyze ownership structures and complexity
- Support multiple UBO algorithms
- Batch and streaming UBO calculations
- Historical UBO tracking

**Key Operations:**
- `CalculateUbo` - Calculate UBO for single entity
- `CalculateUboBatch` - Batch UBO calculations
- `AnalyzeOwnershipStructure` - Structure analysis without full UBO
- `GetUboHistory` - Historical UBO changes

#### 4. **VocabularyService** (`vocabulary_service.proto`)
- Manage domain-specific verb vocabularies
- Register and validate custom verbs
- Domain management and migration
- Vocabulary consistency checking

**Key Operations:**
- `RegisterVerb` - Register new domain verb
- `ValidateVerbUsage` - Check verb usage in context
- `MigrateVerbs` - Move verbs between domains
- `ExportVocabulary` - Export vocabulary definitions

#### 5. **DSLEngineService** (`dsl_engine_service.proto`)
- Main orchestration service
- End-to-end workflow processing
- System health and metrics
- Configuration management
- Development and debugging tools

**Key Operations:**
- `ProcessWorkflow` - Complete workflow processing
- `ParseAndExecute` - Parse and execute DSL programs
- `GetSystemInfo` - System status and capabilities
- `AnalyzeDSL` - Code analysis and suggestions

### Core Types (`dsl_types.proto`)

Shared message types used across all services:

- **Value Types**: `Value`, `ValueList`, `ValueMap` - DSL runtime values
- **AST Types**: `Statement`, `Workflow`, `Program` - Parsed DSL structures
- **Type System**: `DSLType`, `TypeConstraint`, `TypeInfo` - Type definitions
- **Validation**: `ValidationError`, `ValidationWarning` - Error reporting
- **Source Info**: `SourceLocation`, `SourceSpan` - Location tracking

## Code Generation

### Requirements
- [Buf CLI](https://docs.buf.build/installation) for protobuf management
- Go protobuf plugins: `protoc-gen-go`, `protoc-gen-go-grpc`
- Rust protobuf plugins: `prost`, `tonic`

### Generate Code

```bash
# Install buf CLI
curl -sSL "https://github.com/bufbuild/buf/releases/latest/download/buf-$(uname -s)-$(uname -m)" -o "/usr/local/bin/buf"
chmod +x "/usr/local/bin/buf"

# Generate Go and Rust code from proto definitions
cd proto/
buf generate
```

This will generate:
- **Go**: `../go/internal/proto/` - Go structs and gRPC clients/servers
- **Rust**: `../rust/src/proto/` - Rust structs and tonic clients/servers  
- **Docs**: `../docs/proto/` - HTML documentation

### Go Integration

```go
import (
    "context"
    "google.golang.org/grpc"
    pb "github.com/ob-poc/internal/proto/parser"
)

// Create gRPC client
conn, err := grpc.Dial("localhost:50051", grpc.WithInsecure())
if err != nil {
    log.Fatal(err)
}
defer conn.Close()

client := pb.NewParserServiceClient(conn)

// Parse DSL
resp, err := client.ParseDSL(context.Background(), &pb.ParseDSLRequest{
    Source: `(workflow "test" (declare-entity "person1" "person"))`,
    Config: &pb.ParseConfig{
        StrictValidation: true,
        DebugMode: false,
    },
})
```

### Rust Implementation

```rust
use tonic::{transport::Server, Request, Response, Status};
use crate::proto::parser::parser_service_server::{ParserService, ParserServiceServer};
use crate::proto::parser::{ParseDslRequest, ParseDslResponse};

#[derive(Default)]
pub struct ParserServiceImpl;

#[tonic::async_trait]
impl ParserService for ParserServiceImpl {
    async fn parse_dsl(
        &self,
        request: Request<ParseDslRequest>,
    ) -> Result<Response<ParseDslResponse>, Status> {
        let req = request.into_inner();
        
        // Use your existing Rust parser
        let parser = EBNFParser::new();
        match parser.parse_grammar(&req.source) {
            Ok(program) => {
                let response = ParseDslResponse {
                    result: Some(parse_dsl_response::Result::Program(program.into())),
                    metrics: Some(/* ... */),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                let error = ParseFailure {
                    error_message: e.to_string(),
                    // ... other error fields
                };
                Ok(Response::new(ParseDslResponse {
                    result: Some(parse_dsl_response::Result::Failure(error)),
                    metrics: None,
                }))
            }
        }
    }
}
```

## Service Deployment

### Development Setup

```bash
# Start Rust gRPC server
cd rust/
cargo run --bin grpc-server --port 50051

# Go client connects to localhost:50051
cd go/
go run cmd/cli/main.go parse --file example.dsl
```

### Production Deployment

Services can be deployed as:
- **Single Process**: Both Go and Rust in same container
- **Separate Services**: Independent containers with service discovery
- **Kubernetes**: Using service mesh (Istio, Linkerd)
- **Cloud Native**: Cloud Run, EKS, AKS with load balancing

## Testing Strategy

### Unit Tests
- Test each service implementation independently
- Mock gRPC clients for Go business logic tests
- Test Rust service handlers with test clients

### Integration Tests  
- End-to-end workflow processing tests
- Performance benchmarks for large DSL files
- Error handling and recovery testing

### Contract Testing
- Verify protobuf schema compatibility
- Test backward/forward compatibility
- Validate error response formats

## Performance Considerations

### Streaming
- Use streaming RPCs for large files (>10MB)
- Implement backpressure in stream processing
- Handle stream cancellation gracefully

### Caching
- Cache parsed grammars and validation results
- Implement request-level caching for UBO calculations
- Use connection pooling for high-throughput scenarios

### Monitoring
- Instrument all RPCs with metrics (latency, throughput, errors)
- Implement distributed tracing across service boundaries
- Monitor resource usage (CPU, memory, network)

## Error Handling

### Error Types
- **Parse Errors**: Syntax and grammar validation issues
- **Validation Errors**: Semantic and business rule violations  
- **System Errors**: Infrastructure and resource issues
- **User Errors**: Invalid requests and configuration

### Error Responses
All services return structured errors with:
- Error codes for programmatic handling
- Human-readable messages
- Source location information where applicable
- Suggestions for resolution

### Retry Logic
- Implement exponential backoff for transient failures
- Circuit breaker pattern for service degradation
- Graceful fallback for non-critical operations

## Security Considerations

### Authentication
- Support for JWT tokens in gRPC metadata
- Service-to-service authentication with mTLS
- API key authentication for external clients

### Authorization
- Role-based access control (RBAC) for operations
- Fine-grained permissions per service method
- Audit logging for sensitive operations

### Network Security
- TLS encryption for all gRPC communication
- Network policies for service-to-service traffic
- Input validation and sanitization

## Future Enhancements

### Planned Features
- WebAssembly support for browser-based DSL editing
- GraphQL gateway over gRPC services
- Real-time collaboration features for DSL editing
- Plugin system for custom validators and processors

### Scalability
- Horizontal scaling with consistent hashing
- Database sharding for large vocabulary datasets
- Distributed caching with Redis/Memcached
- Event-driven architecture with message queues

## Troubleshooting

### Common Issues
1. **Connection Refused**: Check if Rust gRPC server is running
2. **Proto Compilation Errors**: Ensure buf CLI is properly installed
3. **Type Mismatches**: Regenerate code after proto changes
4. **Performance Issues**: Enable request tracing and profiling

### Debug Tools
- Use `grpcurl` for manual service testing
- Enable gRPC debug logging for connection issues
- Use distributed tracing (Jaeger/Zipkin) for request flows

### Monitoring Endpoints
- Health checks: `/health` endpoint on each service
- Metrics: Prometheus-compatible `/metrics` endpoint  
- Debug: pprof endpoints for Go services

## Contributing

1. **Adding New Services**: Follow existing patterns in service definitions
2. **Schema Changes**: Use backward-compatible changes only
3. **Documentation**: Update this README and generate fresh docs
4. **Testing**: Add comprehensive tests for new functionality

For more details, see the individual service documentation and example code in the `examples/` directory.