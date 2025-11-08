# FORTH-based DSL Execution Engine - Compiler Runtime Plan

## Overview

This document preserves the original Phase 5 plan for implementing a FORTH-based DSL execution engine. This phase was deferred to prioritize web-based AST visualization capabilities, but remains a critical component for the complete DSL system.

## Architecture Vision

The DSL execution system uses a three-layer architecture based on FORTH stack machine principles:

1. **AST → Bytecode Compiler** (`compiler.rs`)
2. **FORTH Stack VM** (`vm.rs`) 
3. **Domain-Specific OpCodes** (Workflow & Graph Engine)

## Implementation Plan

### 1. DSL Execution Architecture Overview

Based on the FORTH discussion, the DSL execution will use domain-specific opcodes rather than simple mathematical operations. The system is designed as a **Workflow and Graph Engine** that handles:

- **Complex Verbs**: `(declare-entity ...)`, `(create-edge ...)`, `(calculate-ubo-prongs ...)`
- **Database Operations**: Entity creation, relationship mapping
- **Domain Logic**: KYC compliance, UBO calculations
- **State Management**: Investigation tracking, monitoring setup

### 2. Execute DSL Implementation
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

### 3. FORTH VM OpCode Architecture
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

### 4. Domain-Specific VM Context
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

### 5. AST to Bytecode Compiler
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

### 6. FORTH Virtual Machine
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
            compliance_results: self.context.get_compliance_results(),
            reports_generated: self.context.get_reports_generated(),
            monitoring_scheduled: self.context.get_monitoring_tasks(),
        })
    }
}
```

### 7. Execution Result Types
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

## Integration with Existing Architecture

The execution phase integrates seamlessly with the existing architecture:

1. **AST Storage** → Used as input to compiler
2. **Domain Context** → Informs execution context setup
3. **Visualization** → Can show execution flow and results
4. **Database** → Execution results stored for audit trail

## Key Design Principles

### FORTH Stack Usage

The FORTH stack passes complex arguments to domain-specific functions:
- `:node-id "company-meridian-global-fund"`
- `:properties { :legal-name "..." :jurisdiction "LU" }`
- `:traversal-rules { :follow-edges [HAS_OWNERSHIP HAS_CONTROL] }`

### Domain-Specific Operations

The VM is not a simple calculator but a **Workflow and Graph Engine** that handles:

- **Entity Management**: Creation, modification, validation
- **Relationship Mapping**: Complex graph operations
- **KYC Compliance**: Regulatory requirement processing
- **UBO Calculations**: Ultimate beneficial ownership analysis
- **Monitoring Setup**: Ongoing compliance monitoring
- **Report Generation**: Regulatory and internal reporting

## Implementation Timeline

When this phase is activated, the estimated implementation timeline is:

- **Week 1**: OpCode definitions, Compiler implementation, Basic VM structure
- **Week 2**: Domain-specific context, Integration testing, Performance optimization

## Testing Strategy

- **Unit Tests**: Individual OpCode execution
- **Integration Tests**: Full AST → Bytecode → Execution pipeline
- **Domain Tests**: KYC, Onboarding, Account Opening workflows
- **Performance Tests**: Large DSL execution benchmarks
- **Security Tests**: Context isolation, permission validation

## Success Criteria

- [ ] Complete AST → Bytecode compilation pipeline
- [ ] FORTH VM with domain-specific OpCode support
- [ ] Execution context with database integration
- [ ] Domain-specific service integration (KYC, UBO, Compliance)
- [ ] Comprehensive execution logging and audit trail
- [ ] Performance benchmarks meeting production requirements
- [ ] Security validation and permission enforcement

## Future Enhancements

- **Parallel Execution**: Multi-threaded OpCode execution
- **Distributed Processing**: Cross-service execution coordination
- **Real-time Monitoring**: Live execution status and metrics
- **Debugging Tools**: Step-through execution, breakpoint support
- **Performance Profiling**: Execution hotspot analysis

---

**Status**: DEFERRED - Awaiting completion of web-based AST visualization (Phase 5)
**Priority**: High - Critical for complete DSL execution capability
**Complexity**: High - Requires deep integration with domain services and database systems