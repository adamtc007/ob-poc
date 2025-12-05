

User: "2"

Agent: → Uses document_id for UK Equity CREST
```

---

## Implementation Architecture

### Protocol Choice: MCP Primary, LSP Adapter

Given agent-first design, **MCP is the primary interface**. LSP is a thin adapter for Zed/IDE use.

```
┌─────────────────────────────────────────────────────────────────┐
│                        Rust Core Library                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ nom parser  │  │ verb        │  │ PostgreSQL              │  │
│  │ + linter    │  │ registry    │  │ connection pool         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              DSL Service (5 operations)                 │    │
│  │   validate | complete_verb | complete_attribute         │    │
│  │   lookup_id | resolve_signature                         │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┴───────────────┐
          │                               │
          ▼                               ▼
┌──────────────────┐            ┌──────────────────┐
│   MCP Server     │            │   LSP Adapter    │
│   (stdio/SSE)    │            │   (tower-lsp)    │
│                  │            │                  │
│  Tools:          │            │  Methods:        │
│  - dsl_validate  │            │  - diagnostic    │
│  - dsl_complete  │            │  - completion    │
│  - dsl_lookup    │            │                  │
│  - dsl_signature │            │                  │
└──────────────────┘            └──────────────────┘
          │                               │
          ▼                               ▼
┌──────────────────┐            ┌──────────────────┐
│  Claude Code     │            │  Zed / VSCode    │
│  Agent           │            │  IDE             │
└──────────────────┘            └──────────────────┘
```

### Crate Structure

```
rust/crates/
├── dsl-core/              # Existing: parser, AST, linter
├── dsl-service/           # NEW: Core service logic
│   ├── src/
│   │   ├── lib.rs
│   │   ├── validate.rs    # Wraps parser + linter
│   │   ├── complete.rs    # Verb/attribute completion
│   │   ├── lookup.rs      # DB queries for IDs
│   │   ├── signature.rs   # Verb parameter resolution
│   │   └── db.rs          # sqlx connection pool
│   └── Cargo.toml
├── dsl-mcp/               # NEW: MCP server binary
│   ├── src/
│   │   └── main.rs        # MCP protocol handler
│   └── Cargo.toml
└── dsl-lsp/               # NEW: LSP server binary (optional)
    ├── src/
    │   └── main.rs        # tower-lsp wrapper
    └── Cargo.toml
```

### Key Dependencies

```toml
[dependencies]
# Core
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "uuid"] }

# MCP (choose one)
# Option A: Use existing MCP crate
# Option B: Simple JSON-RPC over stdio

# LSP (optional)
tower-lsp = "0.20"
```

---

## Database Queries

### lookup_id Queries

**Documents:**
```sql
SELECT 
    d.document_id as id,
    d.document_type || ' - ' || COALESCE(c.legal_name, 'Unknown') as display,
    d.document_type,
    d.created_at
FROM documents d
LEFT JOIN cbus c ON d.cbu_id = c.cbu_id
WHERE d.document_type = $1
  AND ($2::uuid IS NULL OR d.cbu_id = $2)
  AND ($3::text IS NULL OR d.document_name ILIKE '%' || $3 || '%')
ORDER BY d.created_at DESC
LIMIT $4
```

**Entities:**
```sql
SELECT 
    e.entity_id as id,
    e.legal_name as display,
    e.entity_type,
    e.created_at
FROM entities e
WHERE ($1::text IS NULL OR e.entity_type = $1)
  AND ($2::text IS NULL OR e.legal_name ILIKE '%' || $2 || '%')
ORDER BY e.legal_name
LIMIT $3
```

**CBUs:**
```sql
SELECT 
    c.cbu_id as id,
    c.legal_name || ' (' || c.cbu_code || ')' as display,
    c.status,
    c.created_at
FROM cbus c
WHERE ($1::text IS NULL OR c.legal_name ILIKE '%' || $1 || '%')
  AND ($2::text IS NULL OR c.status = $2)
ORDER BY c.legal_name
LIMIT $3
```

---

## MCP Tool Definitions

```json
{
  "tools": [
    {
      "name": "dsl_validate",
      "description": "Validate DSL syntax and semantics. Returns diagnostics with error positions.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "text": { "type": "string", "description": "DSL text to validate" }
        },
        "required": ["text"]
      }
    },
    {
      "name": "dsl_complete",
      "description": "Get completions for verbs or attributes. Use before generating DSL.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "completion_type": { 
            "type": "string", 
            "enum": ["verb", "attribute"],
            "description": "What to complete"
          },
          "prefix": { "type": "string", "description": "Partial text to match" },
          "document_type": { "type": "string", "description": "For attribute completion" },
          "category": { "type": "string", "description": "For verb completion" }
        },
        "required": ["completion_type"]
      }
    },
    {
      "name": "dsl_lookup",
      "description": "Look up real database IDs. ALWAYS use this instead of guessing UUIDs.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "lookup_type": {
            "type": "string",
            "enum": ["document", "entity", "cbu", "case", "attribute"],
            "description": "Type of ID to look up"
          },
          "filters": {
            "type": "object",
            "description": "Filter criteria (document_type, status, client_id, etc.)"
          },
          "search": { "type": "string", "description": "Text search on name/display" },
          "limit": { "type": "integer", "default": 10 }
        },
        "required": ["lookup_type"]
      }
    },
    {
      "name": "dsl_signature",
      "description": "Get verb signature - parameters, types, and requirements.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "verb": { "type": "string", "description": "Verb name" }
        },
        "required": ["verb"]
      }
    }
  ]
}
```

---

## Implementation Tasks

### Phase 1: Core Service
- [ ] Create `dsl-service` crate structure
- [ ] Implement `validate()` - wrap existing parser + linter
- [ ] Implement `complete_verb()` - load from verb registry YAML
- [ ] Implement `complete_attribute()` - load from attribute definitions
- [ ] Implement `resolve_signature()` - parse verb YAML
- [ ] Add sqlx connection pool setup
- [ ] Implement `lookup_id()` with parameterized queries

### Phase 2: MCP Server
- [ ] Create `dsl-mcp` binary crate
- [ ] Implement JSON-RPC over stdio
- [ ] Wire up 4 MCP tools to service layer
- [ ] Add to Claude Code MCP configuration
- [ ] Test agent workflow end-to-end

### Phase 3: LSP Adapter (Optional)
- [ ] Create `dsl-lsp` binary crate  
- [ ] Implement `textDocument/diagnostic` → `validate()`
- [ ] Implement `textDocument/completion` → `complete_verb()` / `complete_attribute()`
- [ ] Configure in Zed settings
- [ ] Test IDE integration

---

## Zed LSP Configuration

Zed supports custom language servers via settings. Two parts:

### 1. Register the language (if not exists)

Create `~/.config/zed/languages/dsl/config.toml`:

```toml
name = "DSL"
grammar = "dsl"
path_suffixes = ["dsl", "obdsl"]
line_comments = ["#", "//"]
block_comment = ["/*", "*/"]
```

### 2. Configure the language server

In `~/.config/zed/settings.json`:

```json
{
  "languages": {
    "DSL": {
      "language_servers": ["dsl-lsp"]
    }
  },
  "lsp": {
    "dsl-lsp": {
      "binary": {
        "path": "/Users/adamtc007/Developer/ob-poc/rust/target/release/dsl-lsp",
        "arguments": []
      },
      "initialization_options": {
        "database_url": "postgresql://localhost/ob-poc",
        "verb_registry": "/Users/adamtc007/Developer/ob-poc/rust/config/verbs.yaml"
      }
    }
  }
}
```

### 3. Optional: Tree-sitter grammar

For syntax highlighting (separate from LSP), you'd need a tree-sitter grammar. Low priority - validation/completion works without it.

### LSP ↔ MCP Reuse

Both use the same `dsl-service` core:

```
┌─────────────────────────────────────────────┐
│              dsl-service (lib)              │
│  validate() complete() lookup() signature() │
└─────────────────────────────────────────────┘
          ▲                       ▲
          │                       │
┌─────────────────┐     ┌─────────────────────┐
│    dsl-mcp      │     │      dsl-lsp        │
│ (agent calls)   │     │  (Zed integration)  │
│                 │     │                     │
│ JSON-RPC/stdio  │     │ tower-lsp protocol  │
└─────────────────┘     └─────────────────────┘
```

The LSP is a thin adapter - most logic lives in `dsl-service`.

### Phase 4: Agent Integration
- [ ] Update agent system prompt with MCP tool guidance
- [ ] Add "lookup before generate" instruction
- [ ] Create agent workflow examples
- [ ] Test hallucination reduction

---

## Success Criteria

1. **Zero UUID hallucination** - Agent always uses `dsl_lookup` before referencing IDs
2. **First-attempt validity** - Generated DSL passes validation 90%+ of time
3. **Human intervention only for selection** - No manual DSL editing required
4. **Sub-100ms response** - Lookup/complete operations fast enough for agent flow

---

## Notes

- MCP is primary interface since agent is primary consumer
- LSP is optional but useful for human debugging in Zed
- Database queries use read-only connection - no mutation through this service
- Verb registry YAML is source of truth for completions/signatures
- Consider caching verb/attribute metadata (reload on SIGHUP)

---

*Document Version: 1.0*  
*Created: December 2024*  
*Status: TODO - Ready for Implementation*
