# Research Macros Implementation Plan

## Overview

Implement the Research Macro system that bridges **fuzzy LLM discovery** → **human review** → **deterministic GLEIF DSL verbs**. This provides a structured way for agents to research clients using web search and LLM reasoning, producing validated JSON that requires human approval before generating executable DSL.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Research Macro  │ ──► │  Human Review   │ ──► │  GLEIF Verbs    │
│ (LLM + search)  │     │  (approve/edit) │     │  (deterministic)│
└─────────────────┘     └─────────────────┘     └─────────────────┘
      fuzzy                   gate                   100% reliable
```

## Key Design Decisions

1. **Follows existing template patterns** - Uses same registry/expander architecture as `ob-templates`
2. **MCP-first** - Exposed as MCP tools for agent use (`research_macro_list`, `research_macro_execute`)
3. **Schema validation** - JSON Schema enforces output structure from LLM
4. **Human review gate** - Results require approval before DSL generation
5. **Handlebars templating** - For prompt rendering and verb template expansion

---

## Files to Create

### 1. Research Macro YAML Config Directory
```
rust/config/macros/research/
├── client-discovery.yaml      # Research institutional client structure
├── ubo-investigation.yaml     # Investigate UBO chain  
└── regulatory-check.yaml      # Check regulatory status/concerns
```

### 2. Research Macro Crate
```
rust/crates/ob-research-macros/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Module exports
│   ├── definition.rs          # ResearchMacroDef, MacroParamDef, ReviewRequirement
│   ├── registry.rs            # ResearchMacroRegistry (load from YAML)
│   ├── executor.rs            # ResearchExecutor (LLM + schema validation)
│   ├── expander.rs            # Handlebars prompt + verb template expansion
│   └── error.rs               # ResearchMacroError
```

### 3. MCP Tool Registration
```
rust/src/mcp/
├── tools.rs                   # Add 3 new tools
├── handlers.rs                # Add handler methods
└── types.rs                   # Add ResearchMacroResult type
```

### 4. RAG Metadata
```
rust/src/session/
└── macro_rag_metadata.rs      # RAG hints for agent discovery
```

---

## Implementation Steps

### Step 1: Create Research Macro Crate Structure

**File: `rust/crates/ob-research-macros/Cargo.toml`**
```toml
[package]
name = "ob-research-macros"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
handlebars = "6.0"
jsonschema = "0.18"
async-trait = "0.1"
tracing = "0.1"
```

### Step 2: Define Core Types

**File: `rust/crates/ob-research-macros/src/definition.rs`**

Key types:
- `ResearchMacroDef` - Full macro definition from YAML
- `MacroParamDef` - Parameter with name, type, required, default, enum
- `ResearchOutput` - Schema + review requirement
- `ReviewRequirement` - Required | Optional | None

### Step 3: Implement Registry

**File: `rust/crates/ob-research-macros/src/registry.rs`**

Pattern: Mirror `ob-templates/src/registry.rs`
- `load_from_dir(path)` - Recursive YAML loading
- `get(name)` - Direct lookup
- `list()` - All macros
- `search(query)` - Text search in name/description

### Step 4: Implement Executor

**File: `rust/crates/ob-research-macros/src/executor.rs`**

```rust
pub struct ResearchExecutor {
    registry: ResearchMacroRegistry,
}

impl ResearchExecutor {
    pub async fn execute(
        &self,
        macro_name: &str,
        params: HashMap<String, Value>,
        llm_client: &dyn LlmClient,
    ) -> Result<ResearchResult>;
}

pub struct ResearchResult {
    pub data: Value,              // LLM response parsed as JSON
    pub schema_valid: bool,       // Passed JSON Schema validation
    pub validation_errors: Vec<String>,
    pub review_required: bool,    // From macro definition
    pub suggested_verbs: Option<String>, // Expanded DSL template
    pub search_quality: Option<String>,  // Self-assessment
}
```

Execution flow:
1. Load macro definition
2. Validate input parameters
3. Render prompt with Handlebars
4. Call `llm_client.chat_json()` (or `chat_with_tool()` for guaranteed JSON)
5. Parse JSON response
6. Validate against output schema
7. Render suggested verbs template with result data
8. Return `ResearchResult`

### Step 5: Register MCP Tools

**File: `rust/src/mcp/tools.rs`** - Add to `get_tools()`:

```rust
// Tool 1: List available research macros
Tool {
    name: "research_macro_list",
    description: "List available research macros with descriptions",
    input_schema: json!({
        "type": "object",
        "properties": {
            "search": { "type": "string" }
        }
    }),
}

// Tool 2: Get macro details
Tool {
    name: "research_macro_get",
    description: "Get full research macro definition with parameters and schema",
    input_schema: json!({
        "type": "object",
        "required": ["macro_name"],
        "properties": {
            "macro_name": { "type": "string" }
        }
    }),
}

// Tool 3: Execute research macro
Tool {
    name: "research_macro_execute",
    description: "Execute research macro with LLM + web search. Returns structured JSON for human review.",
    input_schema: json!({
        "type": "object",
        "required": ["macro_name", "params"],
        "properties": {
            "macro_name": { "type": "string" },
            "params": { "type": "object" }
        }
    }),
}
```

**File: `rust/src/mcp/handlers.rs`** - Add handlers:

```rust
impl ToolHandlers {
    async fn handle_research_macro_list(&self, args: Value) -> Result<Value>;
    async fn handle_research_macro_get(&self, args: Value) -> Result<Value>;
    async fn handle_research_macro_execute(&self, args: Value) -> Result<Value>;
}
```

Add to `dispatch()` match:
```rust
"research_macro_list" => self.handle_research_macro_list(args).await,
"research_macro_get" => self.handle_research_macro_get(args).await,
"research_macro_execute" => self.handle_research_macro_execute(args).await,
```

### Step 6: Create Research Macro YAMLs

**File: `rust/config/macros/research/client-discovery.yaml`**

Based on TODO design - researches institutional client structure with:
- Apex entity (LEI, jurisdiction, listing status)
- Subsidiaries (ManCos, IMs)
- Fund structures
- Regulatory context

### Step 7: Add RAG Metadata

**File: `rust/src/session/macro_rag_metadata.rs`**

```rust
pub struct MacroRagEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub example_prompts: &'static [&'static str],
    pub suggested_follow_up: &'static [&'static str],
}

pub fn research_macro_rag_entries() -> Vec<MacroRagEntry>;
```

### Step 8: Add to Workspace

**File: `rust/Cargo.toml`** - Add to workspace members:
```toml
members = [
    # ... existing
    "crates/ob-research-macros",
]
```

**File: `rust/src/mcp/handlers.rs`** - Add dependency:
```rust
use ob_research_macros::{ResearchMacroRegistry, ResearchExecutor};
```

---

## Testing Plan

### Unit Tests (`rust/crates/ob-research-macros/src/lib.rs`)
- `test_load_research_macro` - YAML loading
- `test_validate_params` - Required/optional param validation
- `test_schema_validation` - JSON Schema enforcement
- `test_prompt_rendering` - Handlebars expansion

### Integration Tests
- `test_research_macro_end_to_end` - Mock LLM, full pipeline
- `test_mcp_tool_registration` - Tools appear in `get_tools()`

### Manual Testing
```bash
# Via dsl_cli or MCP
(research.execute :macro "client-discovery" :client-name "Aviva" :jurisdiction-hint "GB")
```

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `handlebars` | Prompt + verb template rendering |
| `jsonschema` | Output schema validation |
| `serde_yaml` | YAML macro definition loading |
| `ob-agentic` | LlmClient trait access |

---

## Files Modified (Summary)

| File | Action |
|------|--------|
| `rust/Cargo.toml` | Add `ob-research-macros` to workspace |
| `rust/crates/ob-research-macros/*` | **CREATE** - New crate |
| `rust/config/macros/research/*.yaml` | **CREATE** - Macro definitions |
| `rust/src/mcp/tools.rs` | Add 3 tools to `get_tools()` |
| `rust/src/mcp/handlers.rs` | Add 3 handler methods + dispatch cases |
| `rust/src/session/macro_rag_metadata.rs` | **CREATE** - RAG hints |
| `rust/src/session/mod.rs` | Export `macro_rag_metadata` |

---

## Estimated Scope

| Component | Complexity | Est. Lines |
|-----------|------------|------------|
| ob-research-macros crate | Medium | ~500 |
| YAML macro definitions | Low | ~300 |
| MCP tools + handlers | Low | ~150 |
| RAG metadata | Low | ~100 |
| Tests | Medium | ~200 |
| **Total** | | **~1250** |
