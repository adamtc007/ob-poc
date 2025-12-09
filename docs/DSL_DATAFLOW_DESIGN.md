# DSL Dataflow & Execution Precedence Design

## Problem Statement

The OB-POC DSL currently treats programs as flat lists of verb calls. While the parser accepts `@ref` bindings and we support late binding (unresolved references in a valid AST), we have **no formal model of dataflow dependencies** between statements.

This causes real problems:

1. **Agent generates broken DSL** - LLM copies example patterns like `@cbu` without understanding that a producer statement must exist
2. **No workflow-aware autocomplete** - LSP can complete verb names and keywords, but can't suggest "what makes sense next given what you've already defined"
3. **Validation is syntactic only** - CSG linter checks verb/arg validity, but not that `@fund` is defined before `(cbu.assign-role :cbu-id @fund ...)`
4. **Templates are ad-hoc** - Examples are hardcoded in prompt strings, no formal workflow definitions

### The Hidden Dataflow Graph

Every DSL program has an implicit dataflow graph hiding in the `@ref` bindings:

```
(cbu.ensure :name "Pacific Fund" :jurisdiction "LU" :as @fund)
        │
        ▼ produces: CBU (@fund)
        
(entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
        │
        ▼ produces: Entity (@john)
        
(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        ▲              ▲
        │              │
        └──────────────┴── consumes: CBU, Entity
```

This graph is **implicit** - we parse it, execute it, but never formalize it.

---

## Learning from SQL

SQL is a DSL that handles stateful data with well-defined execution precedence. Key lessons:

### 1. Declarative with Implicit Ordering

SQL statements are declarative, but the query planner determines execution order based on dependencies:

```sql
SELECT * FROM orders o
JOIN customers c ON o.customer_id = c.id  -- customers must be "available"
WHERE c.country = 'US'
```

The planner builds a **dependency graph** and executes in valid order. We could do the same - user writes statements in any order, we topologically sort by `@ref` dependencies.

### 2. CTEs (Common Table Expressions) = Named Bindings

```sql
WITH 
  fund AS (INSERT INTO cbus ... RETURNING cbu_id),
  manager AS (INSERT INTO entities ... RETURNING entity_id)
INSERT INTO cbu_entity_roles (cbu_id, entity_id, role)
SELECT fund.cbu_id, manager.entity_id, 'INVESTMENT_MANAGER'
FROM fund, manager;
```

CTEs are **exactly** our `:as @binding` pattern - named intermediate results that downstream statements can reference. SQL validates that CTEs are defined before use.

### 3. Transaction Boundaries

SQL has explicit transaction control (`BEGIN`, `COMMIT`, `ROLLBACK`). Our DSL executes as an implicit transaction, but we might want:

```clojure
;; Atomic workflow - all or nothing
(transaction
  (cbu.ensure :name "Fund" :as @fund)
  (entity.create-proper-person :first-name "John" :as @john)
  (cbu.assign-role :cbu-id @fund :entity-id @john :role "UBO"))
```

### 4. Type System for Columns

SQL has column types and FK constraints. When you write `customer_id`, the schema knows it references `customers.id`. We have this implicitly:

| Our DSL | SQL Equivalent |
|---------|---------------|
| `:cbu-id @fund` | `cbu_id REFERENCES cbus(cbu_id)` |
| `:entity-id @john` | `entity_id REFERENCES entities(entity_id)` |

But we don't **enforce** it. `verbs.yaml` could declare that `:cbu-id` must reference a CBU-typed binding.

### 5. Query Planning = Workflow Planning

SQL optimizers reorder JOINs, push down predicates, etc. We could have a "workflow planner" that:

- Validates dependency order
- Suggests missing steps ("you have a CBU but no entities - add some?")
- Warns about orphans ("@john is created but never used")

---

## Proposed Solution: Extend verbs.yaml with Dataflow Metadata

### Current verbs.yaml Structure

```yaml
domains:
  cbu:
    verbs:
      ensure:
        description: "Create or update a CBU"
        behavior: crud
        args:
          - name: name
            type: string
            required: true
          - name: jurisdiction
            type: string
            required: true
```

### Extended Structure with Dataflow

```yaml
domains:
  cbu:
    verbs:
      ensure:
        description: "Create or update a CBU"
        behavior: crud
        args: [...]
        
        # NEW: Dataflow metadata
        produces:
          - type: cbu
            from_arg: name  # binding name convention: @{name}
            
        consumes: []  # no dependencies - can be first statement
        
      assign-role:
        description: "Assign entity to CBU with role"
        behavior: crud
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: role
            type: string
            required: true
            
        produces: []  # creates relationship, not new entity
        
        consumes:
          - type: cbu
            arg: cbu-id
            required: true
          - type: entity
            arg: entity-id
            required: true

  entity:
    verbs:
      create-proper-person:
        args: [...]
        produces:
          - type: entity
            subtype: proper_person
            from_args: [first-name, last-name]  # @{first_name}_{last_name}
        consumes: []
```

### Type Hierarchy for Consumers

```yaml
# Type definitions (could be separate file or in verbs.yaml header)
types:
  cbu:
    description: "Client Business Unit"
    
  entity:
    description: "Any entity"
    subtypes:
      - proper_person
      - limited_company
      - partnership
      - trust
      
  case:
    description: "KYC case"
    
  workstream:
    description: "Entity workstream within a case"
    parent: case  # workstream requires a case to exist
```

---

## Integration Points

### 1. LSP Autocomplete (Highest Value)

When user types `(` on a new line, LSP can query the dataflow state:

```
Current bindings:
  @fund: cbu
  @john: entity (proper_person)
  
Suggested (dependencies satisfied):
  → cbu.assign-role      requires: cbu ✓, entity ✓
  → kyc-case.create      requires: cbu ✓
  → ubo.register-ubo     requires: cbu ✓, entity ✓
  
Available (no dependencies):
  → cbu.ensure
  → entity.create-*
  
Blocked (missing dependencies):
  → entity-workstream.create  requires: case ✗
```

### 2. CSG Linter - Dataflow Validation

Add a dataflow validation pass after syntax/semantic validation:

```rust
// Pseudo-code
fn validate_dataflow(ast: &Program, verb_registry: &VerbRegistry) -> Vec<DataflowError> {
    let mut available_bindings: HashMap<String, ProducedType> = HashMap::new();
    let mut errors = vec![];
    
    for stmt in &ast.statements {
        let verb_meta = verb_registry.get_dataflow(&stmt.verb);
        
        // Check all consumes are satisfied
        for consume in &verb_meta.consumes {
            if consume.required {
                let arg_value = stmt.get_arg(&consume.arg);
                if let Some(ref_name) = arg_value.as_ref() {
                    if !available_bindings.contains_key(ref_name) {
                        errors.push(DataflowError::UnresolvedDependency {
                            verb: stmt.verb.clone(),
                            arg: consume.arg.clone(),
                            ref_name: ref_name.clone(),
                            expected_type: consume.type.clone(),
                        });
                    }
                }
            }
        }
        
        // Register produces
        if let Some(binding) = &stmt.binding {
            for produce in &verb_meta.produces {
                available_bindings.insert(binding.clone(), produce.clone());
            }
        }
    }
    
    errors
}
```

### 3. Agent Generation - Workflow-Aware Prompts

Instead of hardcoded examples, dynamically build prompts based on:

1. **Current session bindings** → "You have @fund (CBU), @john (person)"
2. **Available next steps** → "You can now: assign-role, add-product, create-case"
3. **Workflow templates** → "Fund onboarding typically follows: ensure → entities → roles → kyc"

```rust
fn build_agent_context(session: &Session, verb_registry: &VerbRegistry) -> String {
    let bindings = session.context.bindings_for_llm();
    let available_verbs = verb_registry.verbs_satisfiable_by(&session.binding_types());
    let suggested_workflow = match_workflow_template(&session.context);
    
    format!(r#"
Current state:
  Bindings: {bindings}
  
Available next verbs (dependencies satisfied):
  {available_verbs}
  
Suggested workflow pattern:
  {suggested_workflow}
"#)
}
```

### 4. Workflow Templates (Higher-Level Abstraction)

For common multi-step scenarios, define workflow templates that reference the verb dataflow:

```yaml
# config/workflows/fund_onboarding.yaml
name: Fund Onboarding
description: Complete fund setup with entities, roles, and KYC
trigger_phrases:
  - "onboard a fund"
  - "create a new fund"
  - "set up fund"

steps:
  - verb: cbu.ensure
    description: "Create the fund CBU"
    required: true
    produces: [cbu]
    
  - verb: entity.create-limited-company
    description: "Create fund management company"
    required: false
    produces: [entity]
    typical_role: INVESTMENT_MANAGER
    
  - verb: entity.create-proper-person
    description: "Create key individuals (directors, UBOs)"
    required: false
    repeat: true  # can have multiple
    produces: [entity]
    typical_roles: [DIRECTOR, BENEFICIAL_OWNER]
    
  - verb: cbu.assign-role
    description: "Link entities to CBU"
    requires: [cbu, entity]
    repeat: true
    
  - verb: kyc-case.create
    description: "Initiate KYC process"
    requires: [cbu]
    produces: [case]
    
  - verb: entity-workstream.create
    description: "Create workstream for each entity"
    requires: [case, entity]
    repeat: true

example: |
  (cbu.ensure :name "Pacific Growth Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
  (entity.create-limited-company :name "Pacific Asset Management Ltd" :jurisdiction "GB" :as @manager)
  (entity.create-proper-person :first-name "Sarah" :last-name "Chen" :as @sarah)
  (entity.create-proper-person :first-name "James" :last-name "Wilson" :as @james)
  (cbu.assign-role :cbu-id @fund :entity-id @manager :role "INVESTMENT_MANAGER")
  (cbu.assign-role :cbu-id @fund :entity-id @sarah :role "DIRECTOR")
  (cbu.assign-role :cbu-id @fund :entity-id @james :role "BENEFICIAL_OWNER")
  (kyc-case.create :cbu-id @fund :case-type "NEW_CLIENT" :as @case)
  (entity-workstream.create :case-id @case :entity-id @sarah :as @ws_sarah)
  (entity-workstream.create :case-id @case :entity-id @james :is-ubo true :as @ws_james)
```

---

## Implementation Plan

### Phase 1: Extend verbs.yaml (Foundation)

1. Add `produces` and `consumes` to verb definitions
2. Define type hierarchy (cbu, entity, case, workstream, etc.)
3. Update `RuntimeVerbRegistry` to load and expose dataflow metadata
4. Write tests validating the metadata

**Files:**
- `rust/config/verbs.yaml` - add dataflow metadata
- `rust/src/dsl_v2/config/types.rs` - extend `VerbDefinition` struct
- `rust/src/dsl_v2/runtime_registry.rs` - load dataflow metadata

### Phase 2: Dataflow Validation (Linter)

1. Add dataflow validation pass to CSG linter
2. Track binding types through AST walk
3. Error on unresolved dependencies
4. Warn on unused bindings (orphans)

**Files:**
- `rust/src/dsl_v2/csg_linter.rs` - add `validate_dataflow()` pass
- `rust/src/dsl_v2/semantic_validator.rs` - integrate with existing validation

### Phase 3: LSP Integration (Autocomplete)

1. Track dataflow state in LSP analysis context
2. Rank completions by dependency satisfaction
3. Show "suggested next steps" based on current bindings
4. Filter verbs with unsatisfied required dependencies

**Files:**
- `rust/crates/dsl-lsp/src/analysis/context.rs` - track binding types
- `rust/crates/dsl-lsp/src/handlers/completion.rs` - dataflow-aware ranking

### Phase 4: Agent Integration (Generation)

1. Remove hardcoded examples from `agent_routes.rs`
2. Build dynamic context from session bindings + dataflow metadata
3. Load workflow templates for few-shot examples
4. Match user intent to workflow template

**Files:**
- `rust/config/workflows/*.yaml` - workflow templates
- `rust/src/api/agent_routes.rs` - dynamic prompt building
- `rust/src/agentic/templates.rs` - template loading and matching

### Phase 5: Workflow Templates (Polish)

1. Define common workflow templates (fund onboarding, custody setup, KYC flow)
2. Intent classification to match user request to template
3. Slot filling from user message
4. Progress tracking ("you're 3/7 steps through fund onboarding")

---

## Open Questions

1. **Ordering strictness** - Should we enforce topological order, or allow any order and sort at execution time?

2. **Optional consumers** - How to model "can use entity if available, but not required"?

3. **Type granularity** - Just `entity` or `entity.proper_person`, `entity.limited_company`?

4. **Workflow selection** - Keyword matching, embedding similarity, or LLM classification?

5. **Cross-reference validation** - Should `:entity-id @john` validate that `@john` is an entity (not a CBU)?

6. **Circular dependencies** - Some workflows are iterative (add entity → assign role → discover UBO → add entity). How to model?

---

## Success Criteria

1. **Agent generates valid DSL** - No more `@cbu` references without a producer
2. **LSP suggests logical next steps** - After `cbu.ensure`, suggest `entity.create-*` and `cbu.assign-role`
3. **Linter catches dataflow errors** - "Error: @manager referenced but not defined"
4. **Templates are validated** - All workflow examples pass parser + dataflow validation
5. **Prompts are dynamic** - No hardcoded DSL in agent code

---

## References

- `rust/config/verbs.yaml` - current verb definitions
- `rust/src/dsl_v2/csg_linter.rs` - current validation
- `rust/crates/dsl-lsp/src/handlers/completion.rs` - current autocomplete
- `rust/src/api/agent_routes.rs` - current hardcoded prompts (lines 1270-1420, 1750-1850)
