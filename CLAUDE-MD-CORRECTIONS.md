# CLAUDE.md Corrections

This document contains all the corrections to apply to CLAUDE.md.

---

## Fix 1: Main Architecture Diagram (replace lines ~55-75)

**FIND THIS (broken):**
```
## Architecture

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
## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   Web UI (localhost:3000)                       │
│  ob-poc-ui (egui/WASM) + ob-poc-web (Axum)                     │
│  5-panel layout: Context | Chat | DSL | Graph | Results         │
│  rust/crates/ob-poc-ui/ + rust/crates/ob-poc-web/              │
└─────────────────────────────────────────────────────────────────┘
```

---

## Fix 2: PostgreSQL line (corrupted Unicode)

**FIND:** `└──────────────��──────────────`
**REPLACE WITH:** `└─────────────────────────────────────────────────────────────────┘`

---

## Fix 3: Parser stage line (corrupted Unicode)

**FIND:** `┌───────────────────────────────────���─────────────────────────────┐`
**REPLACE WITH:** `┌─────────────────────────────────────────────────────────────────┐`

---

## Fix 4: Search Engine YAML stage (corrupted Unicode)

**FIND:** `└───────────────────��─────────────────────────────────────────────┘`
**REPLACE WITH:** `└─────────────────────────────────────────────────────────────────┘`

---

## Fix 5: SearchQuery runtime stage (corrupted Unicode)

**FIND:** `┌───────────────���─────────────────────────────────────────────────┐`
**REPLACE WITH:** `┌─────────────────────────────────────────────────────────────────┐`

---

## Fix 6: Directory structure dag.rs (corrupted Unicode)

**FIND:** `├─── dag.rs`
**REPLACE WITH:** `├── dag.rs`

---

## Fix 7: Directory structure topo_sort.rs (corrupted Unicode)

**FIND:** `├──── topo_sort.rs`
**REPLACE WITH:** `├── topo_sort.rs`

---

## Fix 8: Add Research Module Section (insert after GLEIF section, before Teams section)

**ADD THIS NEW SECTION:**

```markdown
## Research Macros Module

The research module (`rust/src/research/`) enables LLM-powered discovery workflows that bridge fuzzy natural language research to deterministic DSL execution.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   Research Macro Execution                       │
│  "Discover Allianz corporate structure"                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              ResearchRegistry (YAML-driven)                      │
│  Loads macro definitions from config/macros/research/           │
│  rust/src/research/registry.rs                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              ResearchExecutor                                    │
│  - Renders prompt via Handlebars                                │
│  - Calls Claude API with tool_use protocol                      │
│  - Validates JSON output against schema                         │
│  - Extracts and validates LEIs against GLEIF                    │
│  rust/src/research/executor.rs                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Human Review (REQUIRED)                             │
│  Results stored in session, await approval                      │
│  ResearchState: Idle → PendingReview → VerbsReady              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              DSL Verb Generation                                 │
│  Approved data → Handlebars template → GLEIF verbs             │
│  (gleif.enrich :lei "..." :as @entity)                         │
└─────────────────────────────────────────────────────────────────┘
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/research/definition.rs` | ResearchMacroDef, MacroParamDef, ReviewRequirement types |
| `rust/src/research/registry.rs` | YAML loading, tag indexing, fuzzy search |
| `rust/src/research/executor.rs` | Prompt rendering, JSON repair, LEI validation |
| `rust/src/research/llm_client.rs` | Claude API client with tool_use protocol |
| `rust/src/research/error.rs` | ResearchError enum (thiserror) |
| `rust/config/macros/research/*.yaml` | Macro definitions |

### Available Research Macros

| Macro | Description | Output |
|-------|-------------|--------|
| `client-discovery` | Research institutional client structure | Apex entity, subsidiaries, fund structures |
| `ubo-investigation` | Trace ultimate beneficial ownership | Terminus type, ownership chain |
| `regulatory-check` | Regulatory status and compliance history | Sanctions, licenses, enforcement |

### MCP Research Tools

| Tool | Description |
|------|-------------|
| `research_list` | List available macros with optional tag/search filter |
| `research_get` | Get full macro definition including schema |
| `research_execute` | Execute macro, store result in session (PendingReview) |
| `research_approve` | Approve result with optional edits, generate DSL verbs |
| `research_reject` | Reject result, return to Idle state |
| `research_status` | Get current session research state |

### Session State Machine

```
                    ┌─────────────────┐
                    │      Idle       │
                    └────────┬────────┘
                             │ research_execute
                             ▼
                    ┌─────────────────┐
                    │  PendingReview  │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
              ▼                             ▼
     ┌─────────────────┐           ┌─────────────────┐
     │  research_reject │           │ research_approve│
     │  → Idle          │           │ → VerbsReady    │
     └─────────────────┘           └─────────────────┘
```

### Review Requirement Levels

| Level | Behavior |
|-------|----------|
| `required` | Must approve before DSL generation (default) |
| `optional` | Can skip review, auto-approve |
| `none` | No review, immediate DSL generation |

**Design Philosophy:** Research macros are explicitly designed for human-in-the-loop workflows. LLM-discovered data MUST be reviewed before becoming deterministic DSL operations. This prevents hallucinated LEIs or incorrect corporate structures from polluting the database.
```

---

## Fix 9: Add MCP Handlers Section (insert after MCP Tools in DSL Submission section)

**ADD OR EXPAND:**

```markdown
### MCP Handler Architecture

The MCP server (`rust/src/mcp/`) provides tool handlers for Claude Code integration.

**Current Structure:**
```
rust/src/mcp/
├── mod.rs              # Module exports
├── server.rs           # MCP server with stdio transport
├── protocol.rs         # JSON-RPC protocol types
├── tools.rs            # Tool definitions (get_tools())
├── types.rs            # Request/response types
├── session.rs          # SessionStore for state
├── enrichment.rs       # AST enrichment helpers
├── resolution.rs       # Entity resolution helpers
└── handlers/
    ├── mod.rs          # Re-exports
    └── core.rs         # ALL handlers (~2900 lines)
```

**Handler Categories in core.rs:**

| Category | Handlers | Count |
|----------|----------|-------|
| DSL | dsl_validate, dsl_execute, dsl_execute_submission, dsl_bind, dsl_plan, dsl_generate, dsl_lookup, dsl_complete, dsl_signature | 9 |
| CBU | cbu_get, cbu_list | 2 |
| Entity | entity_get, entity_search, schema_info, verbs_list | 4 |
| Workflow | workflow_status, workflow_advance, workflow_transition, workflow_start, resolve_blocker | 5 |
| Template | template_list, template_get, template_expand | 3 |
| Batch | batch_start, batch_add_entities, batch_confirm_keyset, batch_set_scalar, batch_get_state, batch_expand_current, batch_record_result, batch_skip_current, batch_cancel | 9 |
| Research | research_list, research_get, research_execute, research_approve, research_reject, research_status | 6 |

**Note:** `core.rs` is a monolith scheduled for refactoring into domain-specific modules.
```

---

## Fix 10: Add Supporting Modules Section (insert before or after Directory Structure)

**ADD THIS NEW SECTION:**

```markdown
## Supporting Modules

These modules provide cross-cutting functionality used throughout the system.

### Navigation Module (`rust/src/navigation/`)

Natural language navigation commands for the EntityGraph visualization.

| File | Purpose |
|------|---------|
| `commands.rs` | NavCommand enum (LoadCbu, Filter, GoTo, GoUp, GoDown, Find, etc.) |
| `parser.rs` | Nom-based parser for natural language input |
| `executor.rs` | Execute commands against EntityGraph |

**Example Commands:**
- `"show me the Allianz book"` → LoadCbu
- `"go up"` / `"parent"` / `"owner"` → Navigate to parent
- `"filter jurisdiction LU, IE"` → Filter by jurisdiction
- `"find by role director"` → Query by role

### Ontology Module (`rust/src/ontology/`)

Entity taxonomy and lifecycle management loaded from YAML config.

| File | Purpose |
|------|---------|
| `taxonomy.rs` | EntityTaxonomy with type definitions, DB mappings |
| `lifecycle.rs` | State machine validation (is_valid_transition) |
| `semantic_stage.rs` | SemanticStageRegistry for onboarding journey |
| `service.rs` | OntologyService singleton |
| `types.rs` | EntityTypeDef, StateTransition, etc. |

**Config Sources:**
- `entity_taxonomy.yaml` - Entity type definitions
- `semantic_stage_map.yaml` - Onboarding stage definitions
- Verb YAML files - Lifecycle semantics

### Taxonomy Module (`rust/src/taxonomy/`)

Generic three-tier taxonomy pattern: `Type → Operation → Resource`

| File | Purpose |
|------|---------|
| `domain.rs` | TaxonomyDomain trait, TaxonomyMetadata |
| `ops.rs` | TaxonomyOps<D> generic operations, Gap analysis |
| `product.rs` | ProductDomain implementation |
| `instrument.rs` | InstrumentDomain implementation |

**Pattern:**
```
Domain Type ──(M:N)──► Operation ──(M:N)──► Resource Type
                                                  │
                                       CBU Instance Table
```

### Domains Module (`rust/src/domains/`)

Core business domain handlers (trait-based).

| Trait | Purpose |
|-------|---------|
| `DomainHandler` | Base trait for domain-specific operations |
| `KycDomainHandler` | KYC verification workflows |
| `UboDomainHandler` | UBO analysis and calculations |

**Note:** Currently provides trait definitions; implementations route through DSL custom_ops.

### Services Module (`rust/src/services/`)

Service layer implementations for specialized operations.

| Service | Purpose |
|---------|---------|
| `attribute_executor.rs` | Attribute CRUD operations |
| `dictionary_service_impl.rs` | Data dictionary lookups |
| `document_attribute_crud_service.rs` | Document-attribute relationships |
| `document_extraction_service.rs` | Extract structured data from documents |
| `resolution_service.rs` | Entity reference disambiguation |
| `sink_executor.rs` / `source_executor.rs` | Data flow executors |
| `dsl_enrichment.rs` | Source → segments for UI display |

**Note:** Entity search moved to EntityGateway gRPC service.
```

---

## Fix 11: Code Statistics (update the note)

**FIND:**
```markdown
## Code Statistics

As of 2026-01-01:
```

**REPLACE WITH:**
```markdown
## Code Statistics

> **Note:** Statistics below are approximate and may drift. Run `tokei` or `cloc` for current counts.

As of late 2025:
```

---

## Fix 12: Fix egui Section (the truncated response diagram)

The egui section cuts off mid-diagram. Find the section starting with:

```markdown
**The mental model:**

```
┌─────────────────────────────────────────────────────────────────┐
│                         YOUR APP                                 │
```

And ensure it continues with the complete diagram:

```markdown
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
│         │                   │            │ (clicked?)  │        │
│         │                   │            └──────┬──────┘        │
│         │                   │                   │               │
│         │                   ▼                   ▼               │
│         │            ┌─────────────────────────────────┐        │
│         │            │         Action Enum             │        │
│         │            │  Save(id) | Load(id) | Fetch    │        │
│         │            └─────────────┬───────────────────┘        │
│         │                          │                            │
│         │                          ▼                            │
│         │            ┌─────────────────────────────────┐        │
│         │            │      Async Task Spawned         │        │
│         │            └─────────────┬───────────────────┘        │
│         │                          │                            │
│         └──────────────────────────┘                            │
│                   (POST, then refetch)                          │
└─────────────────────────────────────────────────────────────────┘
```

**Key insight:** The UI never mutates server data directly. It requests changes, waits, and refetches. AppState is a **read-only mirror** of server state, updated only via fetch responses.
```

---

## Summary of All Fixes

| # | Type | Description |
|---|------|-------------|
| 1 | Content | Fix main architecture diagram (rust/src/ui/ → crates, 3 panels → 5) |
| 2-7 | Unicode | Fix corrupted box-drawing characters |
| 8 | Content | Add Research Macros Module section |
| 9 | Content | Add MCP Handler Architecture section |
| 10 | Content | Add Supporting Modules section (navigation, ontology, taxonomy, domains, services) |
| 11 | Content | Update code statistics note |
| 12 | Content | Complete truncated egui diagram |
