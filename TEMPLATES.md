# DSL Template System - Implementation Brief

## Overview

Templates are **macro-style constructs** that expand to plain DSL s-expressions at the **agent layer**. The parser, planner, and executor have **zero template awareness** - they only see expanded atoms.

```
User Intent → Agent → Template Expansion → Plain S-expressions → Session → Parser → DAG → Executor
                ↑                                    ↓
          Template Library                   What everything downstream sees
```

## Core Principle

**Macros are the agent's problem. Session sees atoms. Executor sees atoms.**

Templates exist only in:
- Template library (definitions)
- Agent's expansion logic

Templates do NOT exist in:
- Session state
- AST types
- Parser
- Planner/DAG
- Executor

## Syntax Design

### Parameter Conventions

| Prefix | Meaning | Resolution Time |
|--------|---------|-----------------|
| `$param` | Template parameter | Expansion time (compile-time) |
| `@binding` | Runtime binding | Execution time |

### Template Definition

```lisp
(deftemplate standard-onboard ($entity $products)
  "Onboard a CBU with KYC case creation"
  
  (bind @cbu (cbu.ensure 
               entity: $entity 
               products: $products))
  (kyc.create-case 
    cbu: @cbu 
    case-type: "standard"))
```

### Template Invocation

```lisp
(standard-onboard "uuid-of-acme" ["custody" "fund-accounting"])
```

### Expansion Result (what session receives)

```lisp
(bind @cbu (cbu.ensure 
             entity: "uuid-of-acme" 
             products: ["custody" "fund-accounting"]))
(kyc.create-case 
  cbu: @cbu 
  case-type: "standard")
```

## Bulk Operations

### For-Each Syntax

```lisp
(for-each $fund (query.funds account: "allianz-lux")
  (standard-onboard $fund.entity_id $fund.default_products))
```

### Expansion (N blocks for N query results)

```lisp
;; Block 1 - Fund Alpha
(block
  (bind @cbu (cbu.ensure entity: "uuid-001" products: ["custody"]))
  (kyc.create-case cbu: @cbu case-type: "standard"))

;; Block 2 - Fund Beta  
(block
  (bind @cbu (cbu.ensure entity: "uuid-002" products: ["custody"]))
  (kyc.create-case cbu: @cbu case-type: "standard"))

;; ... N blocks
```

## Binding Hygiene

Each template expansion is wrapped in an implicit `block` scope to prevent binding collisions:

```lisp
;; Two invocations:
(standard-onboard "a" [...])
(standard-onboard "b" [...])

;; Expand to separate blocks (no @cbu collision):
(block
  (bind @cbu ...)  ;; scoped to block 1
  (kyc.create-case cbu: @cbu))

(block
  (bind @cbu ...)  ;; scoped to block 2, different @cbu
  (kyc.create-case cbu: @cbu))
```

## Template Storage

### Option A: YAML Files (Recommended for Phase 1)

```yaml
# config/templates/standard-onboard.yaml
name: standard-onboard
description: "Onboard a CBU with KYC case creation"
params:
  - name: entity
    type: uuid
    description: "Entity UUID to onboard"
  - name: products
    type: string[]
    description: "Product codes to provision"
body: |
  (bind @cbu (cbu.ensure entity: $entity products: $products))
  (kyc.create-case cbu: @cbu case-type: "standard")
```

### Option B: Database (Future)

```sql
CREATE TABLE templates (
    id UUID PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE,
    description TEXT,
    params JSONB NOT NULL,      -- [{name, type, description}]
    body TEXT NOT NULL,          -- DSL template source
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
```

## Implementation Phases

### Phase 1: Template Definition & Expansion

**Files to create/modify:**

| File | Purpose |
|------|---------|
| `rust/src/templates/mod.rs` | Module root |
| `rust/src/templates/definition.rs` | `TemplateDefinition` struct, YAML loading |
| `rust/src/templates/expansion.rs` | Parameter substitution logic |
| `rust/src/templates/registry.rs` | Load and cache templates |
| `config/templates/*.yaml` | Template definitions |

**TemplateDefinition struct:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDefinition {
    pub name: String,
    pub description: String,
    pub params: Vec<TemplateParam>,
    pub body: String,  // Raw DSL with $param placeholders
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParam {
    pub name: String,
    pub param_type: ParamType,
    pub description: String,
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    Uuid,
    String,
    StringArray,
    Number,
    Boolean,
}
```

**Expansion logic:**

```rust
impl TemplateDefinition {
    /// Expand template with provided arguments
    /// Returns plain DSL string (s-expressions)
    pub fn expand(&self, args: &[serde_json::Value]) -> Result<String, TemplateError> {
        // 1. Validate arg count matches params
        // 2. Validate arg types
        // 3. Substitute $param with arg values
        // 4. Wrap in (block ...) for hygiene
        // 5. Return expanded DSL string
    }
}
```

### Phase 2: Agent Integration

**Agent service changes:**

```rust
impl AgentService {
    /// Process user message, may involve template expansion
    pub async fn process_message(&self, session_id: Uuid, message: &str) -> AgentResponse {
        // 1. Parse user intent
        // 2. If intent involves template:
        //    a. Load template from registry
        //    b. Resolve parameters (from intent or user context)
        //    c. Expand to plain DSL
        // 3. Add expanded DSL to session
        // 4. Return response with preview
    }
}
```

**Agent context enhancement:**

```rust
pub struct AgentContext {
    // ... existing fields ...
    
    /// Available templates for this context
    pub available_templates: Vec<TemplateSummary>,
}

pub struct TemplateSummary {
    pub name: String,
    pub description: String,
    pub param_names: Vec<String>,
}
```

### Phase 3: Bulk Query Integration

**For-each expansion:**

```rust
pub struct BulkExpansion {
    pub template: TemplateDefinition,
    pub query: String,  // e.g., "query.funds account: \"allianz-lux\""
}

impl BulkExpansion {
    /// Execute query, expand template for each result
    pub async fn expand(&self, gateway: &EntityGateway) -> Result<String, TemplateError> {
        // 1. Execute query via entity gateway
        let results = gateway.execute_query(&self.query).await?;
        
        // 2. Expand template for each result
        let mut blocks = Vec::new();
        for result in results {
            let args = self.extract_args(&result);
            let expanded = self.template.expand(&args)?;
            blocks.push(expanded);
        }
        
        // 3. Return concatenated blocks
        Ok(blocks.join("\n\n"))
    }
}
```

### Phase 4: UI Preview (egui)

```rust
fn template_preview_panel(&mut self, ui: &mut egui::Ui) {
    if let Some(ref expansion) = self.pending_expansion {
        ui.heading("Template Expansion Preview");
        
        ui.horizontal(|ui| {
            ui.label("Template:");
            ui.strong(&expansion.template_name);
        });
        
        ui.horizontal(|ui| {
            ui.label("Instances:");
            ui.strong(format!("{}", expansion.block_count));
        });
        
        ui.separator();
        
        // Paginated block preview
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            let block = &expansion.blocks[self.preview_block_idx];
            ui.code(block);
        });
        
        ui.horizontal(|ui| {
            if ui.button("<").clicked() && self.preview_block_idx > 0 {
                self.preview_block_idx -= 1;
            }
            ui.label(format!("{}/{}", self.preview_block_idx + 1, expansion.block_count));
            if ui.button(">").clicked() && self.preview_block_idx < expansion.block_count - 1 {
                self.preview_block_idx += 1;
            }
        });
        
        ui.separator();
        
        ui.horizontal(|ui| {
            if ui.button("Execute All").clicked() {
                return Some(Action::ExecuteExpansion);
            }
            if ui.button("Cancel").clicked() {
                self.pending_expansion = None;
            }
        });
    }
}
```

## Starter Templates

Create these in `config/templates/`:

### standard-onboard.yaml

```yaml
name: standard-onboard
description: "Standard CBU onboarding with KYC case"
params:
  - name: entity
    type: uuid
    description: "Entity UUID"
  - name: products
    type: string[]
    description: "Product codes"
body: |
  (bind @cbu (cbu.ensure entity: $entity products: $products))
  (kyc.create-case cbu: @cbu case-type: "standard")
```

### enhanced-due-diligence.yaml

```yaml
name: enhanced-due-diligence
description: "EDD onboarding for high-risk entities"
params:
  - name: entity
    type: uuid
    description: "Entity UUID"
  - name: products
    type: string[]
    description: "Product codes"
  - name: risk_factors
    type: string[]
    description: "Identified risk factors"
body: |
  (bind @cbu (cbu.ensure entity: $entity products: $products))
  (kyc.create-case cbu: @cbu case-type: "enhanced")
  (kyc.add-risk-factors case: @cbu.case factors: $risk_factors)
  (kyc.require-documents case: @cbu.case docs: ["source-of-wealth" "beneficial-owner-declaration"])
```

### bulk-book-onboard.yaml

```yaml
name: bulk-book-onboard
description: "Onboard all entities in an account book"
params:
  - name: account
    type: string
    description: "Account code (e.g., allianz-lux)"
  - name: products
    type: string[]
    description: "Product codes for all entities"
body: |
  (for-each $fund (query.funds account: $account)
    (standard-onboard $fund.entity_id $products))
```

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_simple_expansion() {
    let template = TemplateDefinition::load("config/templates/standard-onboard.yaml")?;
    let expanded = template.expand(&[
        json!("uuid-123"),
        json!(["custody", "fund-accounting"]),
    ])?;
    
    assert!(expanded.contains("entity: \"uuid-123\""));
    assert!(expanded.contains("products: [\"custody\", \"fund-accounting\"]"));
    assert!(expanded.starts_with("(block"));
}

#[test]
fn test_param_validation() {
    let template = TemplateDefinition::load("config/templates/standard-onboard.yaml")?;
    
    // Wrong number of args
    let result = template.expand(&[json!("uuid-123")]);
    assert!(matches!(result, Err(TemplateError::ParamCountMismatch { .. })));
    
    // Wrong type
    let result = template.expand(&[json!(123), json!(["custody"])]);
    assert!(matches!(result, Err(TemplateError::ParamTypeMismatch { .. })));
}

#[test]
fn test_hygiene_no_binding_collision() {
    let template = TemplateDefinition::load("config/templates/standard-onboard.yaml")?;
    
    let expanded1 = template.expand(&[json!("uuid-1"), json!(["custody"])])?;
    let expanded2 = template.expand(&[json!("uuid-2"), json!(["custody"])])?;
    
    // Both should be wrapped in separate blocks
    let combined = format!("{}\n{}", expanded1, expanded2);
    assert_eq!(combined.matches("(block").count(), 2);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_agent_template_expansion() {
    let agent = AgentService::new(/* ... */);
    
    let response = agent.process_message(
        session_id,
        "onboard Acme Corp with custody and fund accounting"
    ).await?;
    
    // Agent should have added expanded DSL to session
    let session = session_store.get(session_id)?;
    assert!(session.dsl_source.contains("cbu.ensure"));
    assert!(session.dsl_source.contains("kyc.create-case"));
    
    // No template syntax should remain
    assert!(!session.dsl_source.contains("$entity"));
    assert!(!session.dsl_source.contains("deftemplate"));
}
```

## Success Criteria

1. **Templates load from YAML** - `TemplateRegistry::load_all()` works
2. **Expansion produces valid DSL** - Output parses without errors
3. **Hygiene works** - Multiple expansions don't collide
4. **Agent integrates** - Natural language triggers template expansion
5. **Bulk works** - For-each expands to N blocks
6. **UI previews** - User sees expanded DSL before execution
7. **Executor unchanged** - No template awareness in parser/DAG/executor

## File Checklist

```
rust/src/templates/
├── mod.rs                 # Module exports
├── definition.rs          # TemplateDefinition, TemplateParam, ParamType
├── expansion.rs           # expand(), substitute_params(), wrap_block()
├── registry.rs            # TemplateRegistry, load from YAML
└── error.rs               # TemplateError enum

config/templates/
├── standard-onboard.yaml
├── enhanced-due-diligence.yaml
└── bulk-book-onboard.yaml

rust/src/services/agent_service.rs  # Add template integration
rust/src/api/agent_routes.rs        # Add template list endpoint?

ob-poc-ui/src/panels/template_preview.rs  # egui preview panel
```

## Non-Goals (Not in this implementation)

- Template versioning
- Template permissions/access control
- Template compilation/caching (just string substitution for now)
- Nested template invocations (template calling template)
- Conditional logic in templates (if/else)

Keep it simple: **templates are just parameterized text substitution**.
