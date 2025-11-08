# PLAN: AST Visual Representation Feature - Updated Architecture

## Overview
Implement a function in the Rust DSL manager that builds a visual representation of the AST generated from DSL content. The function will accept a DSL domain and version ID, retrieve the DSL from the database, parse it with NOM, store the AST, and generate visualization data.

## Updated DSL Architecture Understanding

### DSL Domain Structure
The system supports multiple DSL domains with functional states:

#### **Primary Domains**
- **Onboarding** - Client onboarding workflows
- **KYC** - Know Your Customer compliance
- **KYC UBO** - Ultimate Beneficial Owner identification (embedded in KYC)
- **Account Opening** - Account setup processes

#### **DSL Functional States** (Example: KYC Domain)
- `DSL.KYC.Create Case` - Initialize KYC investigation
- `DSL.KYC.Generate UBO` - UBO DSL embedded in main KYC case
- `DSL.KYC.Review and Edit Case` - Review and modification
- `DSL.KYC.Confirm and Compile` - Validation and compilation
- `DSL.KYC.Run` - Execution phase

#### **Shared Grammar**
- All DSLs share the same eBNF grammar at the onboarding platform level
- Domain-specific vocabularies and verbs extend the base grammar
- Consistent parsing pipeline across all domains

### Database Schema Architecture

#### Current Schema Analysis
```sql
-- Existing DSL storage
CREATE TABLE "dsl-ob-poc".dsl_ob (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id VARCHAR(255) NOT NULL,
    dsl_text TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
```

#### Required Schema Updates

##### 1. DSL Domain Management Table
```sql
CREATE TABLE IF NOT EXISTS "dsl-ob-poc".dsl_domains (
    domain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL UNIQUE, -- 'Onboarding', 'KYC', 'KYC_UBO', 'Account_Opening'
    description TEXT,
    base_grammar_version VARCHAR(20) DEFAULT '1.0.0',
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
```

##### 2. Updated DSL Storage Table
```sql
-- Rename/restructure existing dsl_ob table
CREATE TABLE IF NOT EXISTS "dsl-ob-poc".dsl_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES "dsl-ob-poc".dsl_domains (domain_id),
    version_number INTEGER NOT NULL, -- Sequential: 1, 2, 3, etc.
    functional_state VARCHAR(100), -- 'Create_Case', 'Generate_UBO', 'Review_Edit', etc.
    dsl_source_code TEXT NOT NULL,
    compilation_status VARCHAR(50) DEFAULT 'DRAFT', -- 'DRAFT', 'COMPILED', 'ACTIVE', 'DEPRECATED'
    change_description TEXT, -- Change log entry
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (domain_id, version_number)
);
```

##### 3. AST Storage Table (NEW)
```sql
CREATE TABLE IF NOT EXISTS "dsl-ob-poc".parsed_asts (
    ast_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "dsl-ob-poc".dsl_versions (version_id),
    ast_json JSONB NOT NULL, -- Complete AST structure
    parse_metadata JSONB, -- Parser info, timing, validation results
    grammar_version VARCHAR(20), -- eBNF grammar version used
    parsed_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    parser_version VARCHAR(20), -- NOM parser version
    UNIQUE (version_id)
);

CREATE INDEX idx_parsed_asts_version_id ON "dsl-ob-poc".parsed_asts (version_id);
CREATE INDEX idx_parsed_asts_parsed_at ON "dsl-ob-poc".parsed_asts (parsed_at DESC);
```

##### 4. DSL Execution Pipeline Table
```sql
CREATE TABLE IF NOT EXISTS "dsl-ob-poc".dsl_execution_log (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "dsl-ob-poc".dsl_versions (version_id),
    cbu_id VARCHAR(255), -- Client Business Unit context
    execution_phase VARCHAR(50), -- 'COMPILE', 'VALIDATE', 'EXECUTE', 'COMPLETE'
    status VARCHAR(50), -- 'SUCCESS', 'FAILED', 'IN_PROGRESS'
    result_data JSONB, -- Execution results
    error_details JSONB, -- Error information if failed
    started_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    completed_at TIMESTAMPTZ
);
```

## Updated Implementation Plan

### Phase 1: Database Schema Migration & Repository Layer (1.5 weeks)

#### 1.1 Database Migration Scripts
**File**: `sql/migrations/001_dsl_domain_architecture.sql`
- Create new domain-based tables
- Migrate existing `dsl_ob` data to new structure
- Add necessary indexes and constraints

#### 1.2 Domain Repository Layer
**File**: `src/database/dsl_domain_repository.rs`
```rust
pub struct DslDomainRepository {
    pool: sqlx::PgPool,
}

impl DslDomainRepository {
    // Domain management
    pub async fn get_domain_by_name(&self, name: &str) -> Result<DslDomain, DslError>
    pub async fn list_domains(&self) -> Result<Vec<DslDomain>, DslError>
    
    // Version management
    pub async fn get_dsl_version(&self, domain: &str, version: i32) -> Result<DslVersion, DslError>
    pub async fn get_latest_version(&self, domain: &str) -> Result<DslVersion, DslError>
    pub async fn create_new_version(&self, dsl: &NewDslVersion) -> Result<DslVersion, DslError>
    
    // AST management
    pub async fn get_parsed_ast(&self, version_id: &str) -> Result<Option<ParsedAst>, DslError>
    pub async fn store_parsed_ast(&self, ast: &ParsedAst) -> Result<(), DslError>
    pub async fn invalidate_ast(&self, version_id: &str) -> Result<(), DslError>
}
```

#### 1.3 Domain Model Types
**File**: `src/models/domain_models.rs`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslDomain {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub description: Option<String>,
    pub base_grammar_version: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslVersion {
    pub version_id: Uuid,
    pub domain_id: Uuid,
    pub version_number: i32,
    pub functional_state: Option<String>,
    pub dsl_source_code: String,
    pub compilation_status: CompilationStatus,
    pub change_description: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAst {
    pub ast_id: Uuid,
    pub version_id: Uuid,
    pub ast_json: serde_json::Value,
    pub parse_metadata: Option<serde_json::Value>,
    pub grammar_version: String,
    pub parsed_at: DateTime<Utc>,
    pub parser_version: String,
}
```

### Phase 2: Enhanced DSL Manager with AST Storage (1 week)

#### 2.1 Updated DSL Manager
**File**: `src/dsl_manager.rs`
```rust
pub struct DslManager {
    domain_repository: DslDomainRepository,
    parser: DSLParser,
    grammar_version: String,
}

impl DslManager {
    /// Core DSL compilation pipeline
    pub async fn compile_dsl_version(
        &self,
        domain_name: &str,
        version_number: i32,
        force_recompile: bool,
    ) -> DslResult<ParsedAst> {
        // 1. Get DSL version
        let dsl_version = self.domain_repository
            .get_dsl_version(domain_name, version_number)
            .await?;
        
        // 2. Check if AST already exists and is valid
        if !force_recompile {
            if let Some(existing_ast) = self.domain_repository
                .get_parsed_ast(&dsl_version.version_id.to_string())
                .await? {
                return Ok(existing_ast);
            }
        }
        
        // 3. Parse DSL source code to AST
        let ast = self.parser.parse(&dsl_version.dsl_source_code)
            .map_err(|e| DslError::ParseError { message: e.to_string() })?;
        
        // 4. Create parsed AST record
        let parsed_ast = ParsedAst {
            ast_id: Uuid::new_v4(),
            version_id: dsl_version.version_id,
            ast_json: serde_json::to_value(&ast)?,
            parse_metadata: Some(self.create_parse_metadata(&ast)?),
            grammar_version: self.grammar_version.clone(),
            parsed_at: Utc::now(),
            parser_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        
        // 5. Store AST in database
        self.domain_repository.store_parsed_ast(&parsed_ast).await?;
        
        // 6. Update compilation status
        self.update_compilation_status(
            &dsl_version.version_id, 
            CompilationStatus::Compiled
        ).await?;
        
        Ok(parsed_ast)
    }
}
```

### Phase 3: AST Visualization with Domain Context (1.5 weeks)

#### 3.1 Updated Visualization Function
**File**: `src/dsl_manager.rs` (addition)
```rust
impl DslManager {
    /// Main visualization function with updated signature
    pub async fn build_ast_visualization(
        &self,
        domain_name: &str,
        version_number: i32,
        options: Option<VisualizationOptions>,
    ) -> DslResult<ASTVisualization> {
        // 1. Get or compile AST
        let parsed_ast = self.compile_dsl_version(domain_name, version_number, false).await?;
        
        // 2. Deserialize AST from JSON
        let ast: Program = serde_json::from_value(parsed_ast.ast_json)
            .map_err(|e| DslError::InvalidContent { 
                reason: format!("Failed to deserialize AST: {}", e) 
            })?;
        
        // 3. Get domain context
        let domain = self.domain_repository.get_domain_by_name(domain_name).await?;
        let dsl_version = self.domain_repository.get_dsl_version(domain_name, version_number).await?;
        
        // 4. Build visualization with domain context
        let builder = ASTVisualizationBuilder::new()
            .with_domain_context(&domain, &dsl_version)
            .with_layout(options.as_ref().and_then(|o| o.layout).unwrap_or(LayoutType::Tree))
            .with_styling(options.as_ref().and_then(|o| o.styling.clone()).unwrap_or_default())
            .with_filters(options.as_ref().and_then(|o| o.filters.clone()).unwrap_or_default());
            
        let visualization = builder.from_ast(&ast, &parsed_ast)?;
        
        Ok(visualization)
    }
    
    /// Alternative function using version_id directly (for backward compatibility)
    pub async fn build_ast_visualization_by_version_id(
        &self,
        version_id: &str,
        options: Option<VisualizationOptions>,
    ) -> DslResult<ASTVisualization> {
        // Get AST directly by version_id
        let parsed_ast = self.domain_repository
            .get_parsed_ast(version_id)
            .await?
            .ok_or_else(|| DslError::NotFound { 
                id: format!("AST for version_id: {}", version_id) 
            })?;
        
        // Get version info for context
        let dsl_version = self.domain_repository
            .get_dsl_version_by_id(version_id)
            .await?;
            
        // Continue with visualization...
        // (similar to above)
    }
}
```

#### 3.2 Enhanced Visualization Types
**File**: `src/visualization/types.rs`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASTVisualization {
    pub metadata: VisualizationMetadata,
    pub domain_context: DomainContext,
    pub root_node: VisualNode,
    pub edges: Vec<VisualEdge>,
    pub statistics: ASTStatistics,
    pub compilation_info: CompilationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainContext {
    pub domain_name: String,
    pub version_number: i32,
    pub functional_state: Option<String>,
    pub grammar_version: String,
    pub compilation_status: CompilationStatus,
    pub change_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationInfo {
    pub parsed_at: DateTime<Utc>,
    pub parser_version: String,
    pub parse_metadata: Option<serde_json::Value>,
    pub validation_results: Vec<ValidationResult>,
}
```

### Phase 4: Domain-Specific Visualization Features (1 week)

#### 4.1 Domain-Aware Visualization
- **KYC Domain**: Highlight UBO calculation flows, entity relationships
- **Onboarding Domain**: Show workflow progression, decision points
- **Account Opening**: Display requirement validation, approval workflows

#### 4.2 Functional State Visualization
- Show progression through functional states
- Highlight current state in pipeline
- Display state transitions and dependencies

## Updated API Interface

### Primary Function Signature
```rust
pub async fn build_ast_visualization(
    &self,
    domain_name: &str,      // "KYC", "Onboarding", "Account_Opening"
    version_number: i32,    // Sequential version: 1, 2, 3, etc.
    options: Option<VisualizationOptions>,
) -> DslResult<ASTVisualization>
```

### Alternative Signatures
```rust
// Direct version ID lookup (backward compatibility)
pub async fn build_ast_visualization_by_version_id(
    &self,
    version_id: &str,       // UUID of specific version
    options: Option<VisualizationOptions>,
) -> DslResult<ASTVisualization>

// Latest version of domain
pub async fn build_ast_visualization_latest(
    &self,
    domain_name: &str,
    options: Option<VisualizationOptions>,
) -> DslResult<ASTVisualization>
```

### Enhanced Options
```rust
#[derive(Debug, Clone)]
pub struct VisualizationOptions {
    pub layout: Option<LayoutType>,
    pub styling: Option<StylingConfig>,
    pub filters: Option<FilterConfig>,
    pub output_format: Option<OutputFormat>,
    pub include_compilation_info: bool,
    pub include_domain_context: bool,
    pub show_functional_states: bool,
    pub highlight_current_state: bool,
    pub max_depth: Option<usize>,
}
```

## DSL Manager Pipeline Integration

### Complete DSL Lifecycle
```rust
impl DslManager {
    // 1. Create new DSL version
    pub async fn create_dsl_version(&self, domain: &str, source: &str, description: &str) -> DslResult<DslVersion>
    
    // 2. Compile and store AST
    pub async fn compile_dsl_version(&self, domain: &str, version: i32) -> DslResult<ParsedAst>
    
    // 3. Validate AST
    pub async fn validate_ast(&self, version_id: &str) -> DslResult<ValidationResult>
    
    // 4. Generate visualization
    pub async fn build_ast_visualization(&self, domain: &str, version: i32, options: Option<VisualizationOptions>) -> DslResult<ASTVisualization>
    
    // 5. Execute DSL
    pub async fn execute_dsl(&self, domain: &str, version: i32, context: &ExecutionContext) -> DslResult<ExecutionResult>
}
```

## Testing Strategy

### Domain-Specific Test Cases
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_kyc_domain_visualization() {
        let manager = create_test_manager().await;
        
        // Create KYC DSL version
        let kyc_dsl = include_str!("../tests/fixtures/kyc_complete_workflow.dsl");
        let version = manager.create_dsl_version("KYC", kyc_dsl, "Complete KYC workflow").await.unwrap();
        
        // Generate visualization
        let viz = manager.build_ast_visualization("KYC", version.version_number, None).await.unwrap();
        
        // Assert KYC-specific elements
        assert_eq!(viz.domain_context.domain_name, "KYC");
        assert!(viz.root_node.children.iter().any(|id| viz.get_node(id).unwrap().label.contains("UBO")));
    }
}
```

## Migration Plan

### Database Migration
```sql
-- Migration script 001_dsl_domain_architecture.sql
BEGIN;

-- Create new tables
-- (DDL from above)

-- Migrate existing data
INSERT INTO "dsl-ob-poc".dsl_domains (domain_name, description)
VALUES 
    ('Legacy', 'Migrated from old dsl_ob table'),
    ('KYC', 'Know Your Customer workflows'),
    ('Onboarding', 'Client onboarding processes'),
    ('Account_Opening', 'Account opening workflows');

-- Migrate existing DSL records
INSERT INTO "dsl-ob-poc".dsl_versions (domain_id, version_number, dsl_source_code, created_at)
SELECT 
    (SELECT domain_id FROM "dsl-ob-poc".dsl_domains WHERE domain_name = 'Legacy'),
    ROW_NUMBER() OVER (ORDER BY created_at),
    dsl_text,
    created_at
FROM "dsl-ob-poc".dsl_ob;

COMMIT;
```

## Success Criteria

### Functional Requirements
- [ ] Function accepts domain name and version number
- [ ] Retrieves/compiles AST from database with caching
- [ ] Stores parsed AST for reuse
- [ ] Generates domain-aware visualizations
- [ ] Supports all output formats (JSON, DOT, Mermaid, SVG)
- [ ] Handles functional states and compilation pipeline
- [ ] Provides backward compatibility with version IDs

### Performance Requirements
- [ ] AST compilation time < 5 seconds for typical DSLs
- [ ] Visualization generation time < 2 seconds
- [ ] Database query optimization with proper indexing
- [ ] AST caching reduces repeated compilation overhead

### Architecture Requirements
- [ ] Clean separation between domain management and visualization
- [ ] Extensible for new DSL domains
- [ ] Version history and change tracking
- [ ] Integration with existing grammar system
- [ ] Support for functional state progression

### Phase 5: Web-Based AST Visualization (egui/wasm) (2-3 weeks)
Create interactive web-based AST visualization using egui and WebAssembly, providing browser-based access to the domain-specific visualization capabilities developed in Phase 4.

**Previous Phase 5 Content**: The original FORTH-based DSL Execution Engine plan has been moved to `compiler_runtime.md` for future implementation.

#### 5.1 Technology Stack & Architecture

**Frontend Stack**:
- **Framework**: egui (immediate mode GUI)
- **Target**: WebAssembly (wasm32-unknown-unknown)  
- **Build Tool**: trunk for wasm bundling and development server
- **HTTP Client**: reqwest with wasm features

**Backend API**:
- **Server**: warp or axum for REST endpoints
- **Integration**: Direct integration with existing DslManagerV2
- **Serialization**: serde_json for web API communication

**Architecture Overview**:
```
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│   Browser (WASM)    │◄──►│   Web API Server    │◄──►│  AST Visualization  │
│                     │    │                     │    │     Engine          │
│  ┌─────────────────┐│    │ ┌─────────────────┐ │    │ ┌─────────────────┐ │
│  │  egui Web App   ││    │ │ REST Endpoints  │ │    │ │ DslManagerV2    │ │
│  │                 ││    │ │                 │ │    │ │                 │ │
│  │ • AST Renderer  ││    │ │ • /domains      │ │    │ │ • AST Storage   │ │
│  │ • Domain Panel  ││    │ │ • /visualize    │ │    │ │ • Visualization │ │
│  │ • Controls UI   ││    │ │ • /versions     │ │    │ │ • Domain Rules  │ │
│  └─────────────────┘│    │ └─────────────────┘ │    │ └─────────────────┘ │
└─────────────────────┘    └─────────────────────┘    └─────────────────────┘
```

#### 5.2 Web Frontend Implementation

**Core egui Application**:
- Interactive AST node and edge rendering with zoom/pan
- Domain-specific styling and highlighting (KYC, Onboarding, Account Opening)
- Real-time visualization updates via WebSocket or polling
- Export capabilities (SVG, PNG) for documentation

**UI Components**:
- **Domain Panel**: Domain and version selection interface
- **Visualization Controls**: Layout options, filters, styling controls
- **Property Inspector**: Detailed node/edge information display
- **Status Bar**: Connection status, performance metrics

**Interaction Features**:
- Click/drag node selection and positioning
- Hover tooltips with domain-specific information
- Zoom and pan navigation
- Multi-select operations for bulk actions

#### 5.3 Web API Server

**REST Endpoints**:
```rust
// Domain Management
GET /api/domains                           // List available domains
GET /api/domains/{domain}/versions         // List domain versions

// Visualization
GET /api/visualize/{domain}/{version}      // Basic AST visualization
GET /api/visualize/{domain}/{version}/enhanced  // Domain-enhanced visualization
POST /api/visualize/custom                 // Custom visualization options

// Real-time Updates  
WebSocket /ws/visualize/{domain}/{version} // Live visualization updates
```

**Integration Points**:
- Direct integration with existing `DslManagerV2` APIs
- Domain-enhanced visualization pipeline from Phase 4
- Database connectivity for live domain data
- CORS configuration for cross-origin requests

#### 5.4 Key Implementation Components

**AST Renderer** (`ast_renderer.rs`):
- egui-based graph rendering with custom painting
- Force-directed layout algorithms
- Domain-aware node/edge styling
- Interactive selection and manipulation
- Performance optimization for large graphs

**API Client** (`api_client.rs`):
- Async HTTP client with reqwest
- WebAssembly-compatible networking
- Error handling and retry logic
- Caching for improved performance

**State Management**:
- Centralized application state
- Async state updates from API calls
- UI reactivity and data binding
- Persistence of user preferences

#### 5.5 Development Workflow

**Frontend Development**:
1. `trunk serve` - Development server with hot reload
2. Browser-based testing and debugging  
3. WebAssembly optimization for production builds

**Backend API**:
1. `cargo run --bin web-api` - API server development
2. Integration testing with existing DSL manager
3. CORS configuration for local development

**Integrated Testing**:
1. Full-stack visualization pipeline testing
2. Domain-specific visualization validation
3. Performance benchmarking for large ASTs
4. Cross-browser compatibility testing

#### 5.6 Success Criteria

**Functional Requirements**:
- [ ] Interactive web-based AST visualization in browser
- [ ] Domain-aware styling and highlighting (KYC, Onboarding, Account Opening)
- [ ] Real-time domain and version selection
- [ ] Zoom, pan, and selection interactions
- [ ] Export capabilities (SVG, PNG)
- [ ] Responsive design for different screen sizes

**Performance Requirements**:
- [ ] <2s initial load time for typical DSL visualizations
- [ ] <100ms interaction response time (zoom, pan, select)
- [ ] Support for ASTs with 100+ nodes without performance degradation
- [ ] Efficient WebAssembly bundle size (<5MB compressed)

**User Experience**:
- [ ] Intuitive navigation and control interfaces
- [ ] Domain-specific help and documentation
- [ ] Error handling with user-friendly messages
- [ ] Offline capability for cached visualizations

#### 5.7 Future Enhancements

**Advanced Interactions**:
- Collaborative editing with multiple users
- Real-time DSL editing with live AST updates
- Animation of DSL execution flows
- Integration with external documentation systems

**Visualization Extensions**:
- 3D visualization modes for complex hierarchies
- Timeline-based visualization for state changes
- Comparison views for different DSL versions
- Custom theming and branding options

**Mobile Support**:
- Touch-optimized interactions
- Responsive layout adaptation
- Mobile-specific UI patterns
- Progressive Web App (PWA) capabilities