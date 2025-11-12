# Dependency Untangling Plan - Architectural Surgery

**Status**: 🔴 CRITICAL - Multi-Phase Project  
**Estimated Duration**: 4-6 engineering sessions  
**Context Management**: Multiple threads required  

## 🚨 Problem Statement

The OB-POC codebase has **circular dependency hell** preventing clean architecture:

```
CURRENT CIRCULAR DEPENDENCIES:
dsl → parser_ast → parser → dsl                    (FATAL CYCLE)
dsl_manager → dsl → dsl_manager                    (FATAL CYCLE)  
dsl → domains → dsl                                (FATAL CYCLE)
ai → dsl → parser → ai                             (FATAL CYCLE)
```

**Impact**: 
- 500+ compilation errors from tangled imports
- Impossible to implement clean facades
- Architecture drift and maintenance nightmare
- AI agents confused by semantic chaos

## 🎯 Strategic Solution: Strict Dependency Hierarchy

**Core Principle**: Dependencies ONLY flow downward, NEVER create cycles.

```
┌─────────────────────────────────────────────────────────────────┐
│ LEVEL 4: ORCHESTRATION (Top Level)                             │
│ ├── dsl_manager/     (workflow orchestration)                  │
│ ├── agents/          (agentic automation)                      │
│ └── applications/    (CLI, web servers)                        │
└─────────────────────┬───────────────────────────────────────────┘
                      │ depends on
┌─────────────────────▼───────────────────────────────────────────┐
│ LEVEL 3: BUSINESS ENGINES (Business Logic)                     │
│ ├── dsl_engine/     (DSL processing engine)                    │
│ ├── ai_engine/      (AI processing logic)                      │
│ └── query_engine/   (database query logic)                     │
└─────────────────────┬───────────────────────────────────────────┘
                      │ depends on
┌─────────────────────▼───────────────────────────────────────────┐
│ LEVEL 2: INFRASTRUCTURE (Technology Layer)                     │
│ ├── parser/         (nom parsers, grammar)                     │
│ ├── database/       (SQL, persistence)                         │
│ ├── ai_clients/     (OpenAI, Gemini clients)                   │
│ └── network/        (HTTP, gRPC)                               │
└─────────────────────┬───────────────────────────────────────────┘
                      │ depends on
┌─────────────────────▼───────────────────────────────────────────┐
│ LEVEL 1: PURE TYPES (Foundation - NO DEPENDENCIES)             │
│ ├── dsl_types/      (AST, DSL data structures)                 │
│ ├── error_types/    (all error definitions)                    │
│ ├── domain_types/   (business domain definitions)              │
│ └── config_types/   (configuration structures)                 │
└─────────────────────────────────────────────────────────────────┘
```

**CRITICAL RULE**: Level N can ONLY depend on Level N-1 and below. NO UPWARD DEPENDENCIES.

## 📋 Phase-by-Phase Execution Plan

### **PHASE 1: Foundation - Pure Types Extraction**
**Goal**: Create dependency-free type crates  
**Duration**: 1-2 sessions  
**Status**: 🟡 Ready to Start

#### Phase 1.1: Create Type Crates
```bash
# In workspace root
cargo new dsl_types --lib
cargo new error_types --lib  
cargo new domain_types --lib
cargo new config_types --lib

# Update workspace Cargo.toml
[workspace]
members = [
    "rust",           # existing main crate
    "dsl_types",      # NEW: pure DSL data structures
    "error_types",    # NEW: all error definitions
    "domain_types",   # NEW: domain definitions
    "config_types",   # NEW: configuration types
]
```

#### Phase 1.2: Move Leaf Types (One-Struct-at-a-Time Strategy)
**Target Types for First Extraction** (in order):

1. **dsl_types/src/lib.rs**:
   ```rust
   // Pure data structures - NO logic, NO dependencies
   pub struct SourceLocation { /* ... */ }
   pub enum WarningSeverity { /* ... */ }
   pub struct ValidationMetadata { /* ... */ }
   pub struct ProcessingMetadata { /* ... */ }
   ```

2. **error_types/src/lib.rs**:
   ```rust
   // All error types - depend only on basic types
   pub enum DslError { /* ... */ }
   pub enum ParseError { /* ... */ }
   pub enum ValidationError { /* ... */ }
   ```

3. **domain_types/src/lib.rs**:
   ```rust
   // Domain definitions - pure data
   pub enum DomainType { /* ... */ }
   pub struct DomainConfig { /* ... */ }
   pub enum OperationPriority { /* ... */ }
   ```

**Method**: 
1. Pick ONE struct/enum from target list
2. Cut from current location  
3. Paste into appropriate type crate
4. Run `cargo check` - get compiler errors
5. Fix import statements one by one
6. Repeat until compiler happy
7. Move to next type

**Compiler-Guided Surgery**: Let rust compiler generate our TODO list!

### **PHASE 2: Infrastructure Layer Cleanup**
**Goal**: Clean Level 2 dependencies  
**Duration**: 1-2 sessions  
**Status**: 🔴 Blocked by Phase 1

#### Phase 2.1: Parser Cleanup
- Move parser logic to use `dsl_types` instead of internal types
- Remove circular deps: parser → dsl_types (clean)
- Update: `parser/Cargo.toml` add `dsl_types = { path = "../dsl_types" }`

#### Phase 2.2: Database Cleanup  
- Database operations use `dsl_types` for data structures
- Remove database → dsl circular dependency
- Clean persistence layer

#### Phase 2.3: AI Client Cleanup
- AI clients use `dsl_types` for request/response  
- Remove ai → dsl circular dependency
- Pure infrastructure layer

### **PHASE 3: Business Engine Extraction**
**Goal**: Create clean business logic layer  
**Duration**: 1-2 sessions  
**Status**: 🔴 Blocked by Phase 2

#### Phase 3.1: DSL Engine Creation
```bash
cargo new dsl_engine --lib
```

Move DSL business logic (not data structures) to `dsl_engine`:
- DSL processing workflows
- Domain routing logic  
- Validation engines
- Transformation pipelines

**Dependencies**: `dsl_types`, `parser`, `error_types` only

#### Phase 3.2: AI Engine Creation
```bash  
cargo new ai_engine --lib
```

Move AI business logic:
- DSL generation algorithms
- Natural language processing  
- AI service coordination
- Prompt engineering

**Dependencies**: `dsl_types`, `ai_clients`, `error_types` only

### **PHASE 4: Orchestration Layer Finalization**
**Goal**: Clean top-level coordination  
**Duration**: 1 session  
**Status**: 🔴 Blocked by Phase 3

#### Phase 4.1: DSL Manager Cleanup
- Use `dsl_engine` instead of internal dsl module
- Coordinate between engines, not implement logic
- Pure orchestration layer

#### Phase 4.2: Agents Cleanup
- Use `ai_engine` and `dsl_engine`  
- High-level automation workflows
- No internal business logic

### **PHASE 5: Facade Implementation**
**Goal**: Implement clean facades on untangled architecture  
**Duration**: 1 session  
**Status**: 🔴 Blocked by Phase 4

#### Phase 5.1: Apply Facade Pattern
- Now that dependencies are clean, implement facades
- Hide implementation details behind clean interfaces
- Provide semantic boundaries for AI agents

## 🛡️ Anti-Pattern Prevention

### **Circular Dependency Detection**
Before ANY new dependency, run:
```bash
# Check for cycles
cargo tree --duplicates
cargo clippy -- -W clippy::multiple_crate_versions
```

### **Dependency Rules Enforcement**
Create `deny.toml` in workspace root:
```toml
[bans]
deny = [
    # Prevent accidental circular dependencies
    { name = "dsl_types", path = "**dsl_engine**" },  # dsl_types cannot depend on engines
    { name = "error_types", path = "**dsl_engine**" }, # error_types cannot depend on engines
]
```

### **Architecture Validation Tests**
```rust
#[cfg(test)]
mod architecture_tests {
    #[test]
    fn test_no_circular_dependencies() {
        // Automated tests to verify dependency hierarchy
    }
}
```

## 📊 Success Metrics

### **Phase 1 Complete**:
- [ ] All type crates compile with ZERO dependencies (except std)
- [ ] Main crate compiles using external type crates
- [ ] No circular dependencies detected

### **Phase 2 Complete**:
- [ ] Infrastructure layer compiles cleanly  
- [ ] Infrastructure depends only on Level 1
- [ ] Parser, database, ai_clients are pure infrastructure

### **Phase 3 Complete**:
- [ ] Business engines compile cleanly
- [ ] Engines depend only on Levels 1-2
- [ ] Clear separation of data vs. logic

### **Phase 4 Complete**:
- [ ] Orchestration layer depends only on Levels 1-3
- [ ] No business logic in orchestration
- [ ] Clean coordination patterns

### **Phase 5 Complete**:
- [ ] Facade pattern successfully applied
- [ ] Public APIs are minimal and semantic
- [ ] AI agents can understand module boundaries

## 🚨 Context Handoff Instructions

**For Future Sessions**:

1. **Check Current Phase**: Look at this document's status indicators
2. **Verify Dependency Health**: Run `cargo tree` and check for cycles
3. **Follow One-Struct-at-a-Time**: Never move multiple types simultaneously
4. **Update This Document**: Mark progress and update status indicators
5. **Commit Frequently**: Each successful type move should be committed

**Critical Files to Preserve**:
- This plan document
- `workspace/Cargo.toml` (workspace definition)
- Individual crate `Cargo.toml` files (dependency declarations)
- Any `deny.toml` configuration

**Red Flags - Stop Immediately If**:
- Circular dependency detected (`cargo tree --duplicates` shows cycles)
- More than 50 compilation errors (means you moved too much at once)
- Type moved from Level N to Level N+1 (upward movement forbidden)

## 📈 Long-Term Vision

**End State**: Clean, layered architecture where:
- Types are separated from logic
- Dependencies flow only downward  
- Facades provide semantic boundaries
- AI agents can understand and work with clean interfaces
- Architecture is maintainable and extensible

**This is the path to a truly professional, enterprise-ready codebase.**