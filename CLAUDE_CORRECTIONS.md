# CLAUDE.md Corrections

This file contains corrections and additions for CLAUDE.md.
Apply these changes to fix documentation drift.

---

## 1. CORRUPTED UNICODE - Search and Replace

Replace all instances of corrupted box-drawing characters. The pattern is usually `��` or `���`.

**Find and fix these patterns:**
```
└──────────────��──────────────  →  └─────────────────────────────
┌────────────────────���─────────  →  ┌─────────────────────────────
├─�� dag.rs                        →  ├── dag.rs
├─��� topo_sort.rs                 →  ├── topo_sort.rs
├─��� dsl-lsp/                     →  ├── dsl-lsp/
```

**Regex for bulk fix:** `�+` → (delete or replace with appropriate character)

---

## 2. ARCHITECTURE DIAGRAM FIX

**FIND (around line 57-65):**
```
┌─────────────────────────────────────────────────────────────────┐
│                   Web UI (localhost:3000)                       │
│  Server-rendered HTML with embedded JS/CSS                      │
│  Three panels: Chat | DSL Editor | Results                      │
│  rust/src/ui/                                                   │
└─────────────────────────────────────────────────────────────────┘
```

**REPLACE WITH:**
```
┌─────────────────────────────────────────────────────────────────┐
│                   Web UI (localhost:3000)                       │
│  ob-poc-ui (egui/WASM) + ob-poc-web (Axum)                     │
│  5-panel layout: Context | Chat | DSL | Graph | Results         │
│  rust/crates/ob-poc-ui/ + rust/crates/ob-poc-web/              │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. CODE STATISTICS DATE FIX

**FIND:**
```
## Code Statistics

As of 2026-01-01:
```

**REPLACE WITH:**
```
## Code Statistics

> **Note:** Statistics below are approximate and may drift. Run `tokei` or `cloc` for current counts.

As of late 2025:
```

---

## 4. ADD: Research Macro System Section

**INSERT AFTER the "Centralized Entity Lookup Architecture" section (before "Web UI Architecture"):**

```markdown
## Research Macro System

The research module bridges fuzzy LLM discovery with deterministic GLEIF/BODS verbs through a human-in-the-loop approval workflow.

### Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Research Macro  │ ──► │  Human Review   │ ──► │  GLEIF Verbs    │
│ (LLM + search)  │     │  (approve/edit) │     │  (deterministic)│
└─────────────────┘     └─────────────────┘     └─────────────────┘
      fuzzy                   gate                   100% reliable
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/config/macros/research/*.yaml` | Macro definitions (client-discovery, ubo-investigation, etc.) |
| `rust/src/research/definition.rs` | `ResearchMacroDef`, `MacroParamDef`, `ResearchOutput` types |
| `rust/src/research/registry.rs` | YAML loading, macro lookup by name |
| `rust/src/research/executor.rs` | LLM execution with JSON repair, LEI validation |
| `rust/src/research/llm_client.rs` | Claude API client with tool_use protocol |

### Macro Definition Structure

```yaml
# config/macros/research/client-discovery.yaml
name: client-discovery
description: "Discover client structure via GLEIF"
params:
  - name: client_name
    type: string
    required: true
outputs:
  - name: lei
    type: string
  - name: legal_name
    type: string
  - name: jurisdiction
    type: string
review_requirement: required  # always | required | optional
```

### MCP Tools

| Tool | Description |
|------|-------------|
| `research_list` | List available research macros |
| `research_get` | Get macro definition by name |
| `research_execute` | Execute macro with params → pending approval |
| `research_approve` | Approve research result → generates DSL |
| `research_reject` | Reject result with reason |
| `research_status` | Check execution status |

### Approval Workflow

```
research_execute("client-discovery", {client_name: "Allianz"})
    │
    ▼
┌─────────────────────────────────────────┐
│  LLM searches GLEIF, extracts LEI       │
│  Returns: {lei, legal_name, ...}        │
│  Status: PENDING_APPROVAL               │
└─────────────────────────────────────────┘
    │
    ▼
research_approve(execution_id)  OR  research_reject(execution_id, reason)
    │
    ▼
┌─────────────────────────────────────────┐
│  Generates deterministic DSL:           │
│  (gleif.import-entity :lei "...")       │
└─────────────────────────────────────────┘
```
```

---

## 5. ADD: MCP Handlers Structure Section

**INSERT AFTER "Research Macro System" section:**

```markdown
## MCP Server Architecture

The MCP server exposes DSL operations as tools for Claude Code integration.

### Handler Structure

```
rust/src/mcp/
├── handlers/
│   └── core.rs              # All tool handlers (~35 tools, ~2900 lines)
├── protocol.rs              # JSON-RPC protocol types
├── server.rs                # MCP server lifecycle
├── session.rs               # Session state for batch operations
├── tools.rs                 # Tool definitions (names, schemas)
├── types.rs                 # Request/response types
├── enrichment.rs            # DSL enrichment utilities
└── resolution.rs            # Entity resolution helpers
```

### Tool Categories

| Category | Tools | Purpose |
|----------|-------|---------|
| DSL | `dsl_validate`, `dsl_execute`, `dsl_plan`, `dsl_generate` | Parse, validate, execute DSL |
| Submission | `dsl_execute_submission`, `dsl_bind` | Batch execution with bindings |
| Entity | `cbu_get`, `cbu_list`, `entity_get`, `entity_search` | Entity lookup |
| Schema | `verbs_list`, `schema_info`, `dsl_complete`, `dsl_signature` | DSL introspection |
| Workflow | `workflow_status`, `workflow_advance`, `workflow_start` | Workflow orchestration |
| Template | `template_list`, `template_get`, `template_expand` | Template operations |
| Batch | `batch_start`, `batch_add_entities`, `batch_confirm_keyset`, etc. | Template batch execution |
| Research | `research_list`, `research_execute`, `research_approve`, etc. | LLM research macros |

### Entry Point

```rust
// rust/src/mcp/handlers/core.rs
impl ToolHandlers {
    pub async fn handle(&self, name: &str, args: Value) -> ToolCallResult {
        match self.dispatch(name, args).await {
            Ok(v) => ToolCallResult::json(&v),
            Err(e) => ToolCallResult::error(e.to_string()),
        }
    }
}
```
```

---

## 6. ADD: Navigation Module Section

**INSERT AFTER "MCP Server Architecture" section:**

```markdown
## Voice Navigation Module

Natural language navigation for the EntityGraph, designed for voice-first interaction (Blade Runner Esper-style).

### Architecture

```
rust/src/navigation/
├── mod.rs          # Module exports
├── commands.rs     # NavCommand enum with all command types
├── parser.rs       # Nom-based natural language parser
└── executor.rs     # Execute commands against EntityGraph
```

### Command Categories

| Category | Examples |
|----------|----------|
| **Scope** | `load cbu "Fund Name"`, `show the Allianz book` |
| **Filter** | `filter jurisdiction LU, IE`, `show ownership prong`, `as of 2024-01-01` |
| **Navigate** | `go to "Entity"`, `go up`, `go down`, `back`, `forward`, `terminus` |
| **Query** | `find "Name"`, `where is "Person"`, `list children`, `list owners` |
| **Display** | `show path`, `show tree 3`, `expand cbu`, `zoom in`, `fit` |

### Usage

```rust
use ob_poc::navigation::{parse_nav_command, NavCommand, NavExecutor};

let input = "show me the Allianz book";
match parse_nav_command(input) {
    Ok((_, cmd)) => {
        let executor = NavExecutor::new(&graph);
        let result = executor.execute(cmd);
    }
    Err(e) => eprintln!("Parse error: {:?}", e),
}
```
```

---

## 7. ADD: Ontology & Taxonomy Modules Section

**INSERT AFTER "Voice Navigation Module" section:**

```markdown
## Ontology & Taxonomy Modules

### Ontology Module (`rust/src/ontology/`)

Manages entity type definitions and lifecycle state machines.

| File | Purpose |
|------|---------|
| `taxonomy.rs` | `EntityTaxonomy` - loads from `entity_taxonomy.yaml` |
| `lifecycle.rs` | State machine validation (`is_valid_transition`, `valid_next_states`) |
| `semantic_stage.rs` | `SemanticStageRegistry` - onboarding journey stages |
| `service.rs` | `OntologyService` - global access via `ontology()` |

**Config sources:**
- `config/entity_taxonomy.yaml` - Entity type definitions with DB mappings
- `config/agent/semantic_stages.yaml` - Onboarding stage definitions

### Taxonomy Module (`rust/src/taxonomy/`)

Generic three-tier taxonomy pattern: `Type → Operation → Resource`

```
Domain Type ──(M:N)──► Operation ──(M:N)──► Resource Type
                                                 │
                                      CBU Instance Table
```

| File | Purpose |
|------|---------|
| `domain.rs` | `TaxonomyDomain` trait, `TaxonomyMetadata` |
| `ops.rs` | `TaxonomyOps<D>` - generic operations for any domain |
| `product.rs` | `ProductDomain` - product taxonomy implementation |
| `instrument.rs` | `InstrumentDomain` - instrument taxonomy implementation |

**Key operations:** `Discovery` (find gaps), `Gap` (missing coverage), `ProvisionResult` (provision resources)
```

---

## 8. ADD: Services Module Section

**INSERT AFTER "Ontology & Taxonomy Modules" section:**

```markdown
## Services Module (`rust/src/services/`)

Business logic implementations for the DSL execution layer.

| File | Purpose |
|------|---------|
| `attribute_executor.rs` | Attribute validation and execution |
| `resolution_service.rs` | Entity reference disambiguation |
| `document_extraction_service.rs` | Document content extraction |
| `document_attribute_crud_service.rs` | Document-attribute linking |
| `dsl_enrichment.rs` | DSL source → UI segments for highlighting |
| `sink_executor.rs` | Composite sink operations |
| `source_executor.rs` | Composite source operations |

**Note:** Entity search has moved to EntityGateway gRPC service. The services module focuses on document/attribute operations and DSL enrichment.
```

---

## 9. ADD: Domains Module Note

**INSERT AFTER "Services Module" section:**

```markdown
## Domains Module (`rust/src/domains/`)

> **Status:** Stub implementation. Defines `DomainHandler` trait but uses `StubDomainHandler` for most domains.

The domains module provides a trait-based abstraction for domain-specific DSL handling:

```rust
#[async_trait]
pub trait DomainHandler: Send + Sync {
    fn domain_name(&self) -> &str;
    fn supported_verbs(&self) -> Vec<&str>;
    async fn validate_dsl(&self, verb: &str, properties: &PropertyMap) -> Result<(), String>;
    async fn process_operation(&self, verb: &str, properties: &PropertyMap) -> Result<DomainResult, String>;
}
```

**Registered domains:** `kyc`, `onboarding`, `ubo`, `isda`, `entity`, `products`, `documents`, `cbu`, `crud`, `attr`

**Note:** Actual DSL execution goes through `GenericCrudExecutor` (YAML-driven) or `custom_ops/` plugins. This module is for future domain-specific logic.
```

---

## 10. FIX: Directory Structure - Add Missing Modules

**FIND in Directory Structure section:**
```
│   ├── src/                        # Main crate (database-integrated)
│   │   ├── api/                    # REST API routes
```

**ADD these lines after `api/` section:**
```
│   │   ├── domains/                # Domain handler trait (stub implementations)
│   │   ├── navigation/             # Voice navigation commands + parser
│   │   ├── ontology/               # Entity taxonomy + lifecycle state machines
│   │   ├── taxonomy/               # Generic Type→Operation→Resource pattern
│   │   ├── services/               # Document/attribute services, DSL enrichment
```

---

## 11. FIX: Truncated egui Section

The egui "Philosophy" section appears complete in the file. However, ensure the section ends properly with:

```markdown
### Philosophy: Why These Patterns Exist

egui is an **immediate mode** GUI...

[... existing content ...]

**The mental model:**

```
┌─────────────────────────────────────────────────────────────────┐
│                         YOUR APP                                 │
│                                                                  │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │
│   │   Server    │───▶│  AppState   │───▶│    egui     │        │
│   │   (truth)   │    │  (mirror)   │    │  (painter)  │        │
│   └─────────────┘    └─────────────┘    └─────────────┘        │
│         ▲                   │                   │               │
│         │                   │                   ▼               │
│         │                   │            ┌─────────────┐        │
│         │                   │            │  Response   │        │
│         │                   │            │  (actions)  │        │
│         └───────────────────┼────────────┴─────────────┘        │
│                POST/fetch   │                                    │
└─────────────────────────────────────────────────────────────────┘
```

**Key insight:** The server is truth. AppState is a cache. egui is just a painter that returns what the user clicked.
```

---

## Summary Checklist

- [ ] Fix corrupted Unicode characters (regex: `�+`)
- [ ] Update architecture diagram (rust/src/ui/ → rust/crates/ob-poc-ui/)
- [ ] Fix panel count (3 → 5)
- [ ] Add "Research Macro System" section
- [ ] Add "MCP Server Architecture" section
- [ ] Add "Voice Navigation Module" section
- [ ] Add "Ontology & Taxonomy Modules" section
- [ ] Add "Services Module" section
- [ ] Add "Domains Module" note
- [ ] Update directory structure with missing modules
- [ ] Fix code statistics date
- [ ] Verify egui section is complete

---

## Application Instructions

1. Open CLAUDE.md in an editor with good regex support
2. Run find/replace for corrupted Unicode: `�+` → empty or appropriate char
3. Apply each section fix in order
4. Verify box-drawing characters render correctly
5. Commit with message: `docs: fix CLAUDE.md drift - architecture, modules, unicode`
