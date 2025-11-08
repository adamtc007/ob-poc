# Web UI Architecture Plan - Full Rust Stack with gRPC

## Overview

This document outlines the architecture for a full Rust stack web application for AST visualization, using EGUI/WASM frontend, Axum web server, and gRPC communication with the DslManagerV2 backend.

**Timeline**: 3-4 weeks  
**Priority**: High - Critical for web-based AST visualization  
**Dependencies**: DslManagerV2 gRPC service implementation

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│   Browser (WASM)    │    │   Axum Web Server   │    │   gRPC Server       │
│                     │    │   (Gateway/Proxy)   │    │   (Backend)         │
│                     │    │                     │    │                     │
│  ┌─────────────────┐│    │ ┌─────────────────┐ │    │ ┌─────────────────┐ │
│  │     EGUI        ││    │ │ Static Files    │ │    │ │  DslManagerV2   │ │
│  │   Frontend      ││    │ │ (WASM + Assets) │ │    │ │   gRPC Service  │ │
│  │                 ││    │ └─────────────────┘ │    │ │                 │ │
│  │ • Domain Panel  ││    │ ┌─────────────────┐ │    │ │ • Database      │ │
│  │ • AST Viewer    ││◄──►│ │  REST API       │◄│◄──►│ │ • AST Engine    │ │
│  │ • Controls      ││    │ │  Gateway        │ │    │ │ • Visualization │ │
│  │ • Visualizer    ││    │ │                 │ │    │ │ • Domain Rules  │ │
│  └─────────────────┘│    │ │ • GET /api/*    │ │    │ └─────────────────┘ │
└─────────────────────┘    │ │ • gRPC Client   │ │    └─────────────────────┘
                           │ └─────────────────┘ │    
                           └─────────────────────┘    
```

### Component Responsibilities

#### 1. Browser (EGUI/WASM)
- **Purpose**: Interactive AST visualization interface
- **Technology**: egui compiled to WASM
- **Responsibilities**:
  - Render AST visualizations (trees, graphs, hierarchical)
  - Domain selection and version management UI
  - Interactive controls (zoom, pan, filters)
  - Real-time visualization updates
  - Domain-specific highlighting and styling

#### 2. Axum Web Server (Gateway/Proxy)
- **Purpose**: Static file serving + REST API gateway
- **Technology**: Axum + Tonic gRPC client
- **Responsibilities**:
  - Serve WASM application and static assets
  - Provide REST API endpoints for frontend
  - Convert REST requests to gRPC calls
  - Handle CORS and security
  - Basic error handling and logging

#### 3. gRPC Server (Backend)
- **Purpose**: Core business logic and data management
- **Technology**: Tonic + DslManagerV2
- **Responsibilities**:
  - Domain and version management
  - AST generation and caching
  - Database operations
  - Visualization logic
  - Domain-specific rules and highlighting

## Project Structure

```
ob-poc/
├── rust/                    # Existing core library
├── grpc-server/            # gRPC backend service
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs         # gRPC server entry point
│   │   ├── services/       # gRPC service implementations
│   │   │   ├── mod.rs
│   │   │   ├── dsl_service.rs
│   │   │   └── visualization_service.rs
│   │   └── proto/          # Generated protobuf code
│   ├── proto/              # Protobuf definitions
│   │   ├── dsl.proto
│   │   └── visualization.proto
│   └── build.rs            # Protobuf compilation
├── web-server/             # Axum gateway server
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs         # Axum server entry point
│   │   ├── handlers/       # REST API handlers
│   │   │   ├── mod.rs
│   │   │   ├── domains.rs
│   │   │   └── visualizations.rs
│   │   ├── grpc_client.rs  # gRPC client wrapper
│   │   └── types.rs        # API types
│   └── static/             # Static assets (WASM output goes here)
├── web-frontend/           # EGUI/WASM frontend
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs         # WASM entry point
│   │   ├── app.rs          # Main EGUI application
│   │   ├── components/     # UI components
│   │   │   ├── mod.rs
│   │   │   ├── domain_panel.rs
│   │   │   ├── ast_viewer.rs
│   │   │   └── controls.rs
│   │   ├── api_client.rs   # HTTP API client
│   │   └── types.rs        # Frontend types
│   ├── index.html          # HTML template
│   └── Trunk.toml          # Trunk configuration
└── proto/                  # Shared protobuf definitions
    ├── dsl.proto
    └── visualization.proto
```

## Implementation Phases

### Phase 1: gRPC Server Setup (1 week)

#### 1.1 Protobuf API Design
```protobuf
// proto/dsl.proto
service DslService {
  rpc ListDomains(ListDomainsRequest) returns (ListDomainsResponse);
  rpc GetDomain(GetDomainRequest) returns (GetDomainResponse);
  rpc ListDomainVersions(ListDomainVersionsRequest) returns (ListDomainVersionsResponse);
  rpc GetVersion(GetVersionRequest) returns (GetVersionResponse);
}

// proto/visualization.proto
service VisualizationService {
  rpc GenerateAstVisualization(GenerateAstVisualizationRequest) returns (GenerateAstVisualizationResponse);
  rpc GenerateDomainVisualization(GenerateDomainVisualizationRequest) returns (GenerateDomainVisualizationResponse);
}
```

#### 1.2 gRPC Service Implementation
- Wrap DslManagerV2 in gRPC services
- Implement error handling and logging
- Add configuration management
- Setup database connection pooling

#### 1.3 Testing
- Unit tests for gRPC services
- Integration tests with database
- Performance testing

### Phase 2: Axum Gateway Server (4-5 days)

#### 2.1 REST API Design
```
GET /api/domains                               # List all domains
GET /api/domains/{id}                         # Get domain details
GET /api/domains/{id}/versions                # List domain versions
GET /api/domains/{id}/versions/{version_id}/ast    # Get AST visualization
GET /api/domains/{id}/versions/{version_id}/domain # Get domain visualization
```

#### 2.2 Implementation
- Setup Axum server with static file serving
- Implement gRPC client integration
- Create REST endpoint handlers
- Add CORS and middleware
- Error handling and logging

#### 2.3 Testing
- API endpoint testing
- gRPC client integration testing
- Static file serving verification

### Phase 3: EGUI Frontend (1.5 weeks)

#### 3.1 Core Application Structure
- Setup EGUI application with proper state management
- Implement HTTP API client
- Create main UI layout with panels

#### 3.2 UI Components
- **Domain Panel**: Domain selection, version browsing
- **AST Viewer**: Canvas-based AST rendering with zoom/pan
- **Controls Panel**: Layout options, filters, styling controls
- **Status Bar**: Loading states, error messages

#### 3.3 Visualization Engine
- AST rendering with different layouts (tree, graph, hierarchical)
- Interactive features (node selection, highlighting)
- Domain-specific styling and colors
- Export capabilities

#### 3.4 Build Setup
- Trunk configuration for WASM compilation
- Asset optimization and bundling
- Development server integration

### Phase 4: Integration & Polish (3-4 days)

#### 4.1 End-to-End Testing
- Full stack integration testing
- Cross-browser compatibility
- Performance optimization

#### 4.2 Error Handling
- Comprehensive error states in UI
- Retry mechanisms
- Offline handling

#### 4.3 Documentation
- API documentation
- User guide
- Development setup guide

## Technology Stack Details

### Frontend (EGUI/WASM)
```toml
[dependencies]
egui = "0.24"
eframe = { version = "0.24", features = ["default", "wgpu"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["json"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = "0.3"
```

### Web Server (Axum)
```toml
[dependencies]
axum = { version = "0.7", features = ["macros"] }
tonic = { version = "0.10", features = ["tls"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "fs", "trace"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### gRPC Server (Tonic)
```toml
[dependencies]
tonic = { version = "0.10", features = ["tls"] }
prost = "0.12"
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres"] }
tracing = "0.1"
tracing-subscriber = "0.3"
ob-poc = { path = "../rust" }

[build-dependencies]
tonic-build = "0.10"
```

## API Communication Flow

### 1. Domain Loading Flow
```
Frontend                Web Server              gRPC Server
   │                        │                       │
   │──GET /api/domains──────►│                       │
   │                        │──ListDomains()───────►│
   │                        │                       │──Database Query
   │                        │◄────DomainsResponse───│
   │◄────JSON Response──────│                       │
   │                        │                       │
```

### 2. AST Visualization Flow
```
Frontend                Web Server              gRPC Server
   │                        │                       │
   │──GET /api/.../ast──────►│                       │
   │                        │──GenerateAst()───────►│
   │                        │                       │──AST Generation
   │                        │                       │──Visualization Logic
   │                        │◄────VisualizationResp─│
   │◄────JSON Response──────│                       │
   │                        │                       │
```

## Configuration

### Environment Variables
```bash
# gRPC Server
DATABASE_URL=postgresql://localhost:5432/ob-poc
GRPC_PORT=50051
LOG_LEVEL=info

# Web Server  
GRPC_ENDPOINT=http://localhost:50051
WEB_PORT=3000
STATIC_DIR=./static
CORS_ORIGINS=*

# Frontend (build-time)
API_BASE_URL=http://localhost:3000/api
```

### Development Workflow
```bash
# Terminal 1: Start gRPC server
cd grpc-server && cargo run

# Terminal 2: Start web server  
cd web-server && cargo run

# Terminal 3: Build and serve frontend
cd web-frontend && trunk serve
```

## Security Considerations

1. **CORS**: Properly configured for development and production
2. **Input Validation**: All API inputs validated on both REST and gRPC layers  
3. **Error Handling**: No sensitive information exposed in error responses
4. **Rate Limiting**: Consider implementing for production use
5. **Authentication**: Placeholder for future auth integration

## Performance Considerations

1. **WASM Size**: Minimize bundle size through feature flags
2. **Caching**: Implement visualization caching in gRPC server
3. **Lazy Loading**: Load domains and versions on demand
4. **Connection Pooling**: gRPC client connection reuse
5. **Database**: Query optimization and connection pooling

## Deployment Strategy

### Development
- Local services running on different ports
- Hot reloading for frontend development
- Docker Compose for easy setup

### Production  
- Container orchestration (Docker/Kubernetes)
- Separate scaling of components
- Load balancing for web servers
- Database connection management

## Success Criteria

1. ✅ Full Rust stack implementation
2. ✅ Interactive AST visualization in browser
3. ✅ Domain-specific highlighting and styling
4. ✅ Real-time updates and smooth interactions
5. ✅ Comprehensive error handling
6. ✅ Development workflow efficiency
7. ✅ Production deployment readiness

This architecture provides clear separation of concerns, excellent performance through gRPC, and a maintainable full-Rust codebase suitable for both development and production deployment.
