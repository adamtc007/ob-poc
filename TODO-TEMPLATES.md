# TODO: Agent REPL Template & Bulk Integration

## Context

Templates exist as a concept (see TEMPLATES.md). This TODO covers integrating template macro expansion into the agent REPL session, then extending to bulk operations.

**Core principle:** Agent expands templates → plain s-expressions → session. Parser/executor never see templates.

---

## CRITICAL: Entity References, Not UUIDs

**Templates output entity names/search strings, NOT UUIDs.**

The existing DSL pipeline already handles entity resolution:

```
Template Expansion → Plain DSL (with entity names)
                           ↓
                     Parser → AST with unresolved refs
                           ↓
                     Linter → collects entity refs needing resolution
                           ↓
                     Entity Resolution (search, modal, user clarifies)
                           ↓
                     AST populated with UUIDs
                           ↓
                     Compile/DAG/Execute
```

**CORRECT template param/body:**
```yaml
params:
  - name: cbu_name          # String - entity search term
    type: string
  - name: director_name
    type: string
body: |
  (kyc-case.create :cbu-id (cbu "$cbu_name") :case-type "standard")
  (entity.create-proper-person :first-name "$director_first" :last-name "$director_last")
```

**WRONG - do NOT put UUIDs in templates:**
```yaml
params:
  - name: cbu_id            # ❌ UUID - bypasses resolution
    type: uuid
body: |
  (kyc-case.create :cbu-id "$cbu_id" ...)  # ❌ Compiler chokes on literal UUID
```

**Why:** Template-generated DSL is identical to user-typed DSL. The existing linter walks the AST, finds unresolved entity refs like `(cbu "Acme Corp")`, triggers entity search/resolution, and populates UUIDs. Templates don't need special handling.

**ParamType enum should NOT include Uuid:**
```rust
pub enum ParamType {
    String,       // Entity names, search terms
    StringArray,  // Product lists, doc types
    Number,
    Boolean,
    // NO Uuid - entities resolved by existing pipeline
}
```

---

## Phase 1: Template Infrastructure

### 1.1 Create Template Module

- [ ] Create `rust/src/templates/mod.rs`
- [ ] Create `rust/src/templates/definition.rs`
  ```rust
  pub struct TemplateDefinition {
      pub name: String,
      pub description: String,
      pub params: Vec<TemplateParam>,
      pub body: String,  // DSL with $param placeholders
  }
  
  pub struct TemplateParam {
      pub name: String,
      pub param_type: ParamType,
      pub description: String,
      pub default: Option<serde_json::Value>,
  }
  
  pub enum ParamType {
      Uuid, String, StringArray, Number, Boolean
  }
  ```
- [ ] Create `rust/src/templates/error.rs` - `TemplateError` enum
- [ ] Add `mod templates;` to `rust/src/lib.rs`

### 1.2 Template Loading

- [ ] Create `rust/src/templates/registry.rs`
  ```rust
  pub struct TemplateRegistry {
      templates: HashMap<String, TemplateDefinition>,
  }
  
  impl TemplateRegistry {
      pub fn load_from_dir(path: &Path) -> Result<Self, TemplateError>;
      pub fn get(&self, name: &str) -> Option<&TemplateDefinition>;
      pub fn list(&self) -> Vec<&TemplateDefinition>;
  }
  ```
- [ ] Create `config/templates/` directory
- [ ] Create starter template: `config/templates/standard-onboard.yaml`
  ```yaml
  name: standard-onboard
  description: "Standard CBU onboarding with KYC case"
  params:
    - name: entity_name
      type: string
      description: "Entity name or search term"
    - name: products
      type: string[]
      description: "Product codes"
  body: |
    (bind @cbu (cbu.ensure :entity (entity "$entity_name") :products $products))
    (kyc-case.create :cbu-id @cbu :case-type "standard")
  ```

### 1.3 Template Expansion

- [ ] Create `rust/src/templates/expansion.rs`
  ```rust
  impl TemplateDefinition {
      pub fn expand(&self, args: &[serde_json::Value]) -> Result<String, TemplateError> {
          // 1. Validate arg count
          // 2. Validate arg types (NO UUID type - use String for entity names)
          // 3. Substitute $param → value (strings quoted, arrays as [...])
          // 4. Wrap in (block ...) for hygiene
          // 5. Return expanded DSL (with entity refs, NOT UUIDs)
      }
  }
  ```
- [ ] Handle nested $param.field access (e.g., `$fund.entity_name`)
- [ ] Proper value serialization:
  - Strings: quoted `"value"`
  - Arrays: `["a" "b" "c"]`
  - Numbers: unquoted
  - Booleans: `true` / `false`
- [ ] **NO UUID handling** - entities are `(entity "name")` refs, resolved later by linter

### 1.4 Unit Tests

- [ ] Test simple expansion
- [ ] Test param validation (count, types)
- [ ] Test hygiene (multiple expansions don't collide)
- [ ] Test nested field access

---

## Phase 2: Agent REPL Integration

### 2.1 Add Registry to AppState

- [ ] Add `TemplateRegistry` to server `AppState`
- [ ] Load templates on startup
- [ ] Reload on config change (optional)

### 2.2 Agent Template Awareness

- [ ] Add template list to agent context
  ```rust
  pub struct AgentContext {
      // ... existing ...
      pub available_templates: Vec<TemplateSummary>,
  }
  ```
- [ ] Update agent system prompt to know about templates
- [ ] Agent can suggest template usage based on user intent

### 2.3 Template Expansion in Agent Service

- [ ] Add method to `AgentService`:
  ```rust
  pub async fn expand_template(
      &self,
      template_name: &str,
      args: &[serde_json::Value],
  ) -> Result<String, AgentError>;
  ```
- [ ] Agent calls this when user intent matches template
- [ ] Expanded DSL added to session (plain s-expressions)

### 2.4 Agent Commands for Templates

- [ ] `(template.list)` - show available templates
- [ ] `(template.show name)` - show template definition
- [ ] `(template.apply name arg1 arg2 ...)` - expand and add to session

### 2.5 Integration Tests

- [ ] User says "onboard Acme with custody" → agent uses template
- [ ] Expanded DSL appears in session
- [ ] No template syntax in session (pure atoms)

---

## Phase 3: Bulk Operations

### 3.1 Query Integration

- [ ] Agent can execute entity queries:
  ```rust
  pub async fn query_entities(
      &self,
      query: &str,  // e.g., "funds account: allianz-lux"
  ) -> Result<Vec<serde_json::Value>, AgentError>;
  ```
- [ ] Query goes through EntityGateway
- [ ] Results returned as JSON values

### 3.2 For-Each Expansion

- [ ] Add to expansion.rs:
  ```rust
  pub fn expand_for_each(
      template: &TemplateDefinition,
      results: &[serde_json::Value],
      field_mapping: &HashMap<String, String>,  // $param → $.result_field
  ) -> Result<String, TemplateError>;
  ```
- [ ] Each result produces one (block ...)
- [ ] All blocks concatenated

### 3.3 Bulk Template Definition

- [ ] Create `config/templates/bulk-book-onboard.yaml`:
  ```yaml
  name: bulk-book-onboard
  description: "Onboard all entities in account book"
  params:
    - name: account
      type: string
    - name: products
      type: string[]
  query: "funds account: $account"
  per_item:
    template: standard-onboard
    mapping:
      entity: $.entity_id
      products: $products  # passthrough from bulk params
  ```

### 3.4 Agent Bulk Commands

- [ ] Natural language: "onboard all allianz lux funds with custody"
- [ ] Agent:
  1. Identifies bulk operation
  2. Executes query (47 results)
  3. Expands template × 47
  4. Shows preview (count, sample)
  5. On confirm, adds to session

### 3.5 Preview Before Commit

- [ ] Agent shows expansion preview:
  ```
  Template: standard-onboard
  Query: funds account: "allianz-lux"
  Matched: 47 entities
  
  Preview (1 of 47):
  (block
    (bind @cbu (cbu.ensure entity: "uuid-001" ...))
    (kyc.create-case cbu: @cbu ...))
  
  Say "execute" to add all 47 blocks to session.
  ```
- [ ] User confirms before bulk add

---

## Phase 4: UI Support (egui)

### 4.1 Template List Panel

- [ ] Show available templates
- [ ] Click to see definition
- [ ] "Use this template" → starts agent flow

### 4.2 Bulk Preview Panel

- [ ] When bulk expansion pending:
  - Template name
  - Query
  - Result count
  - Paginated block preview
  - Execute / Cancel buttons

### 4.3 Session Stack Grouping (Optional)

- [ ] Visual grouping of statements from same expansion
- [ ] Collapse/expand bulk groups
- [ ] "Delete all from this expansion" action

---

## Known Issues from Test Harness (2025-12-16)

Reference: `templates-issues.md`

| Issue | Description | Fix | Priority |
|-------|-------------|-----|----------|
| **Issue 1** | Missing verbs in compiler (`doc-request.create`, `kyc-case.escalate`, etc.) | Add verb implementations to `compiler.rs` | High |
| **Issue 2** | Template uses `:name` but verb requires `:first-name`/`:last-name` | Update template YAML to match verb signatures | High |
| **Issue 3** | UUID parameters not resolved | **NOT AN ISSUE** - templates should output `(entity "name")` refs, not UUIDs. Existing linter/resolution handles this. Fix templates to use entity names. | N/A |
| **Issue 4** | Undefined symbol `@workstream` | Cascading error - resolves when Issue 1 fixed | Low |
| **Issue 5** | `request-documents` needs iteration | Agent calls template N times, OR simplify to single doc per template | Low |

**Key insight for Issue 3:** The compiler error "cannot resolve Literal(Uuid(...)) to entity key" means the template is wrong, not the compiler. Templates should never contain UUIDs - they should use entity refs like `(entity "Acme Corp")` or `(cbu "Acme Corp")` which the existing resolution pipeline handles.

---

## Verification Gates

### Gate 1: Template Infrastructure
```bash
cargo test --package ob-poc templates::
# All template unit tests pass
```

### Gate 2: Agent Integration
```bash
# In agent REPL:
> template.list
# Shows available templates

> template.apply standard-onboard "Acme Corp" ["custody"]
# Session now contains expanded DSL with (entity "Acme Corp") refs
# Linter triggers entity resolution
```

### Gate 3: Bulk Operations
```bash
# In agent REPL:
> onboard all funds in allianz-lux with custody
# Shows preview: 47 entities matched
> execute
# Session contains 47 blocks
```

### Gate 4: End-to-End
```bash
# Full flow:
1. User: "bulk onboard allianz lux book with custody and fund accounting"
2. Agent: Shows preview (N entities)
3. User: "looks good"
4. Agent: Adds expanded blocks to session
5. User: "execute"
6. Executor: Runs all blocks (DAG orders within each block)
7. UI: Shows results
```

---

## File Checklist

```
rust/src/templates/
├── mod.rs                 [ ]
├── definition.rs          [ ]
├── expansion.rs           [ ]
├── registry.rs            [ ]
└── error.rs               [ ]

config/templates/
├── standard-onboard.yaml       [ ]
├── enhanced-due-diligence.yaml [ ]
└── bulk-book-onboard.yaml      [ ]

rust/src/services/agent_service.rs  [ ] Add template methods
rust/src/state.rs                   [ ] Add TemplateRegistry to AppState

ob-poc-ui/src/panels/
├── template_list.rs       [ ]
└── bulk_preview.rs        [ ]
```

---

## Non-Goals (Out of Scope)

- Template versioning
- Template permissions
- Nested templates (template calling template)
- Conditional logic in templates
- Template compilation/optimization

Keep it simple: **parameterized text substitution + query iteration**.
