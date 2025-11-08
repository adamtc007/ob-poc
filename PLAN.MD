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

### Phase 5: FORTH-based DSL Execution Engine (2 weeks) - **DEFERRED**
*Implementation will be deferred until AST visualization and AST storage are fully implemented and working.*

#### 5.1 DSL Execution Architecture Overview
Based on the FORTH discussion, the DSL execution will use a three-layer architecture:

1. **AST → Bytecode Compiler** (`compiler.rs`)
2. **FORTH Stack VM** (`vm.rs`) 
3. **Domain-Specific OpCodes** (Workflow & Graph Engine)

#### 5.2 Execute DSL Implementation
**File**: `src/dsl_manager.rs` (addition)
```rust
impl DslManager {
    /// Execute compiled DSL using FORTH-based VM
    pub async fn execute_dsl(
        &self,
        domain_name: &str,
        version_number: i32,
        context: &ExecutionContext, // Data Dictionary
    ) -> DslResult<ExecutionResult> {
        
        // 1. GET THE AST
        // Leverages the new AST storage architecture
        let parsed_ast_record = self.compile_dsl_version(domain_name, version_number, false).await?;
        let ast: AstNode = serde_json::from_value(parsed_ast_record.ast_json)?;

        // 2. THE "BRIDGE" (COMPILE AST TO BYTECODE)
        // Maps AST to FORTH vocabulary using domain-specific opcodes
        let bytecode = match compiler::compile_ast_to_bytecode(&ast) {
            Ok(bc) => bc,
            Err(e) => return Err(DslError::CompileError(e.to_string())),
        };

        // 3. THE "VM" (EXECUTE BYTECODE)
        // FORTH stack machine with workflow/graph engine capabilities
        let mut vm = ForthVM::new(context);
        match vm.execute(&bytecode) {
            Ok(result) => Ok(result),
            Err(e) => Err(DslError::RuntimeError(e.to_string())),
        }
    }
}
```

#### 5.3 FORTH VM OpCode Architecture
**File**: `src/execution/opcodes.rs`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpCode {
    // Stack operations
    Push(Value),
    Pop,
    Dup,
    Swap,
    
    // Domain-specific workflow opcodes (not simple math!)
    CallDeclareEntity,     // Pops entity data, executes DB INSERT
    CallCreateEdge,        // Pops edge data, creates relationships
    CallKycCollect,        // Executes KYC data collection
    CallCalculateUBO,      // Runs UBO calculation algorithms
    CallGenerateReport,    // Generates compliance reports
    CallScheduleMonitoring, // Sets up ongoing monitoring
    
    // Control flow
    Branch(usize),
    BranchIf(usize),
    Call(String),
    Return,
    
    // Data dictionary operations
    LoadAttribute(String),
    StoreAttribute(String),
    ValidateAttribute(String),
}
```

#### 5.4 Domain-Specific VM Context
**File**: `src/execution/context.rs`
```rust
/// Execution context containing data dictionary and database connections
#[derive(Debug)]
pub struct ExecutionContext {
    // Data dictionary with runtime values
    pub attributes: HashMap<String, AttributeValue>,
    
    // Database connections for entity operations
    pub db_pool: sqlx::PgPool,
    
    // Domain-specific services
    pub kyc_service: Box<dyn KycService>,
    pub ubo_calculator: Box<dyn UboCalculator>,
    pub compliance_checker: Box<dyn ComplianceChecker>,
    
    // Execution tracking
    pub execution_log: Vec<ExecutionEvent>,
    pub current_step: Option<String>,
    
    // Security context
    pub user_permissions: UserPermissions,
    pub audit_trail: AuditTrail,
}

impl ExecutionContext {
    /// Execute domain-specific operations based on OpCode
    pub async fn execute_opcode(&mut self, opcode: &OpCode, stack: &mut Vec<Value>) -> Result<(), ExecutionError> {
        match opcode {
            OpCode::CallDeclareEntity => {
                // Pop entity data from FORTH stack
                let properties = stack.pop().unwrap().as_map()?;
                let entity_type = stack.pop().unwrap().as_string()?;
                let node_id = stack.pop().unwrap().as_string()?;
                
                // Execute database operation
                self.declare_entity(node_id, entity_type, properties).await?;
                
                // Push result back to stack
                stack.push(Value::Boolean(true));
            },
            OpCode::CallCreateEdge => {
                // Pop edge data from stack
                let evidenced_by = stack.pop().unwrap().as_list()?;
                let properties = stack.pop().unwrap().as_map()?;
                let edge_type = stack.pop().unwrap().as_string()?;
                let to = stack.pop().unwrap().as_string()?;
                let from = stack.pop().unwrap().as_string()?;
                
                // Execute graph operation
                self.create_edge(from, to, edge_type, properties, evidenced_by).await?;
                stack.push(Value::Boolean(true));
            },
            OpCode::CallCalculateUBO => {
                // Pop UBO calculation parameters
                let traversal_rules = stack.pop().unwrap().as_map()?;
                let threshold = stack.pop().unwrap().as_number()?;
                let target = stack.pop().unwrap().as_string()?;
                
                // Execute UBO calculation
                let result = self.ubo_calculator.calculate(target, threshold, traversal_rules).await?;
                stack.push(Value::Map(result));
            },
            // ... other domain-specific opcodes
            _ => return Err(ExecutionError::UnsupportedOpCode(format!("{:?}", opcode))),
        }
        Ok(())
    }
}
```

#### 5.5 AST to Bytecode Compiler
**File**: `src/execution/compiler.rs`
```rust
pub struct ASTCompiler {
    opcodes: Vec<OpCode>,
    symbol_table: HashMap<String, usize>,
}

impl ASTCompiler {
    /// Compile AST to FORTH bytecode
    pub fn compile_ast_to_bytecode(ast: &AstNode) -> Result<Vec<OpCode>, CompileError> {
        let mut compiler = ASTCompiler::new();
        compiler.compile_node(ast)?;
        Ok(compiler.opcodes)
    }
    
    fn compile_node(&mut self, node: &AstNode) -> Result<(), CompileError> {
        match node {
            AstNode::Statement(Statement::DeclareEntity { id, entity_type, properties }) => {
                // Push arguments in reverse order (FORTH stack)
                self.opcodes.push(OpCode::Push(Value::String(id.clone())));
                self.opcodes.push(OpCode::Push(Value::String(entity_type.clone())));
                self.opcodes.push(OpCode::Push(Value::Map(properties.clone())));
                
                // Call the domain-specific opcode
                self.opcodes.push(OpCode::CallDeclareEntity);
            },
            AstNode::Statement(Statement::CreateEdge { from, to, edge_type, properties, evidenced_by }) => {
                // Push arguments for edge creation
                self.opcodes.push(OpCode::Push(Value::String(from.clone())));
                self.opcodes.push(OpCode::Push(Value::String(to.clone())));
                self.opcodes.push(OpCode::Push(Value::String(edge_type.to_string())));
                self.opcodes.push(OpCode::Push(Value::Map(properties.clone())));
                self.opcodes.push(OpCode::Push(Value::List(
                    evidenced_by.iter().map(|s| Value::String(s.clone())).collect()
                )));
                
                self.opcodes.push(OpCode::CallCreateEdge);
            },
            AstNode::Statement(Statement::CalculateUbo { entity_id, properties }) => {
                // Extract UBO calculation parameters
                let target = entity_id.clone();
                let threshold = properties.get("threshold")
                    .and_then(|v| v.as_number())
                    .unwrap_or(25.0);
                let traversal_rules = properties.get("traversal_rules")
                    .and_then(|v| v.as_map())
                    .cloned()
                    .unwrap_or_default();
                
                self.opcodes.push(OpCode::Push(Value::String(target)));
                self.opcodes.push(OpCode::Push(Value::Number(threshold)));
                self.opcodes.push(OpCode::Push(Value::Map(traversal_rules)));
                
                self.opcodes.push(OpCode::CallCalculateUBO);
            },
            // ... compile other AST node types
            _ => return Err(CompileError::UnsupportedAstNode(format!("{:?}", node))),
        }
        Ok(())
    }
}
```

#### 5.6 FORTH Virtual Machine
**File**: `src/execution/vm.rs`
```rust
pub struct ForthVM {
    stack: Vec<Value>,
    call_stack: Vec<usize>,
    instruction_pointer: usize,
    context: ExecutionContext,
}

impl ForthVM {
    pub fn new(context: ExecutionContext) -> Self {
        Self {
            stack: Vec::new(),
            call_stack: Vec::new(),
            instruction_pointer: 0,
            context,
        }
    }
    
    /// Execute bytecode using FORTH stack machine
    pub async fn execute(&mut self, bytecode: &[OpCode]) -> Result<ExecutionResult, ExecutionError> {
        self.instruction_pointer = 0;
        
        while self.instruction_pointer < bytecode.len() {
            let opcode = &bytecode[self.instruction_pointer];
            
            match opcode {
                OpCode::Push(value) => {
                    self.stack.push(value.clone());
                },
                OpCode::Pop => {
                    self.stack.pop()
                        .ok_or(ExecutionError::StackUnderflow)?;
                },
                OpCode::Dup => {
                    let value = self.stack.last()
                        .ok_or(ExecutionError::StackUnderflow)?
                        .clone();
                    self.stack.push(value);
                },
                // Domain-specific opcodes executed through context
                OpCode::CallDeclareEntity | 
                OpCode::CallCreateEdge | 
                OpCode::CallCalculateUBO |
                OpCode::CallKycCollect |
                OpCode::CallGenerateReport |
                OpCode::CallScheduleMonitoring => {
                    self.context.execute_opcode(opcode, &mut self.stack).await?;
                },
                OpCode::Branch(target) => {
                    self.instruction_pointer = *target;
                    continue;
                },
                OpCode::BranchIf(target) => {
                    let condition = self.stack.pop()
                        .ok_or(ExecutionError::StackUnderflow)?;
                    if condition.as_bool().unwrap_or(false) {
                        self.instruction_pointer = *target;
                        continue;
                    }
                },
                // ... other opcodes
            }
            
            self.instruction_pointer += 1;
        }
        
        // Return execution results
        Ok(ExecutionResult {
            final_stack: self.stack.clone(),
            execution_log: self.context.execution_log.clone(),
            entities_created: self.context.get_entities_created(),
            relationships_created: self.context.get_relationships_created(),
        })
    }
}
```

#### 5.7 Execution Result Types
**File**: `src/execution/types.rs`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub final_stack: Vec<Value>,
    pub execution_log: Vec<ExecutionEvent>,
    pub entities_created: Vec<EntityId>,
    pub relationships_created: Vec<RelationshipId>,
    pub compliance_results: Vec<ComplianceResult>,
    pub reports_generated: Vec<ReportId>,
    pub monitoring_scheduled: Vec<MonitoringTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub description: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    EntityDeclared,
    EdgeCreated,
    UboCalculated,
    ComplianceChecked,
    ReportGenerated,
    MonitoringScheduled,
    Error,
}
```

#### 5.8 Integration with Existing DSL Manager
The execution phase integrates seamlessly with the existing architecture:

1. **AST Storage** → Used as input to compiler
2. **Domain Context** → Informs execution context setup
3. **Visualization** → Can show execution flow and results
4. **Database** → Execution results stored for audit trail

#### 5.9 Key Insights from KYC DSL Example
The `kyc_ubo_example_dsl.txt` reveals that the VM is not a simple calculator but a **Workflow and Graph Engine**:

- **Complex Verbs**: `(declare-entity ...)`, `(create-edge ...)`, `(calculate-ubo-prongs ...)`
- **Database Operations**: Entity creation, relationship mapping
- **Domain Logic**: KYC compliance, UBO calculations
- **State Management**: Investigation tracking, monitoring setup

#### 5.10 FORTH Stack Usage
The FORTH stack passes complex arguments to domain-specific functions:
- `:node-id "company-meridian-global-fund"`
- `:properties { :legal-name "..." :jurisdiction "LU" }`
- `:traversal-rules { :follow-edges [HAS_OWNERSHIP HAS_CONTROL] }`

This updated plan reflects the proper domain-based DSL architecture with AST storage, compilation pipeline integration, and FORTH-based execution engine.