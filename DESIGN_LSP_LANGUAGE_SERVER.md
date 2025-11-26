# Design: DSL Language Server Protocol (LSP) Implementation

**Created:** 2025-11-25  
**Updated:** 2025-11-26  
**Status:** IMPLEMENTED  
**Priority:** P2 — Developer Experience  
**Scope:** LSP server for IDE integration (Zed, VS Code)  

---

## Implementation Status

| Component | Status | Location |
|-----------|--------|----------|
| **LSP Server Core** | IMPLEMENTED | `rust/crates/dsl-lsp/src/server.rs` |
| **Completion Handler** | IMPLEMENTED | `rust/crates/dsl-lsp/src/handlers/completion.rs` |
| **Hover Handler** | IMPLEMENTED | `rust/crates/dsl-lsp/src/handlers/hover.rs` |
| **Diagnostics Handler** | IMPLEMENTED | `rust/crates/dsl-lsp/src/handlers/diagnostics.rs` |
| **Go-to-Definition** | IMPLEMENTED | `rust/crates/dsl-lsp/src/handlers/goto_definition.rs` |
| **Signature Help** | IMPLEMENTED | `rust/crates/dsl-lsp/src/handlers/signature.rs` |
| **Document Symbols** | IMPLEMENTED | `rust/crates/dsl-lsp/src/handlers/symbols.rs` |
| **Schema Cache** | IMPLEMENTED | `rust/src/forth_engine/schema/cache.rs` |
| **DB Loading** | IMPLEMENTED | `SchemaCache::load_from_db()` |
| **Zed Extension** | IMPLEMENTED | `rust/crates/dsl-lsp/zed-extension/` |
| **Tree-sitter Grammar** | IMPLEMENTED | `rust/crates/dsl-lsp/tree-sitter-dsl/` |
| **Lookup Tables Migration** | IMPLEMENTED | `sql/migrations/018_lsp_lookup_tables.sql` |

---

## Executive Summary

The LSP server provides IDE integration for the Onboarding DSL:
- Syntax highlighting and error detection
- **Smart completions with human-readable picklists** (Option A)
- Go-to-definition for `@symbol` references
- Hover documentation for verbs
- Signature help while typing

**Option A Decision:** Display human-readable names, insert codes, runtime resolves to UUIDs.

```
User sees:  "Certificate of Incorporation"
DSL gets:   "CERT_OF_INCORP"
Runtime:    Looks up UUID from document_types table
```

---

## Part 1: Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           IDE (Zed / VS Code)                               │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     │ LSP Protocol (JSON-RPC over stdio)
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          dsl-lsp (Rust binary)                              │
│                                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │   Parser     │  │   Analyzer   │  │ VerbRegistry │  │ SchemaCache  │    │
│  │              │  │              │  │              │  │              │    │
│  │ • Tokenize   │  │ • Symbol     │  │ • 28 verbs   │  │ • Doc types  │    │
│  │ • Parse      │  │   table      │  │ • ArgSpecs   │  │ • Attributes │    │
│  │ • AST        │  │ • Type check │  │ • Examples   │  │ • Roles      │    │
│  │ • Errors     │  │ • References │  │              │  │ • Currencies │    │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘    │
│         │                 │                 │                 │             │
│         └─────────────────┴─────────────────┴─────────────────┘             │
│                                     │                                       │
│                                     ▼                                       │
│                    ┌─────────────────────────────┐                          │
│                    │    LSP Response Builder     │                          │
│                    │  • Completions              │                          │
│                    │  • Diagnostics              │                          │
│                    │  • Hover                    │                          │
│                    │  • Go-to-definition         │                          │
│                    └─────────────────────────────┘                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     │ (via SchemaCache::load_from_db)
                                     ▼
                              ┌─────────────┐
                              │  PostgreSQL │
                              │  (lookups)  │
                              └─────────────┘
```

---

## Part 2: Type Consistency Chain

The LSP relies on a consistent type chain from database to DSL:

### Database → Rust Mapping

| RefType | DB Table | Code Column | Display Column | Status |
|---------|----------|-------------|----------------|--------|
| `DocumentType` | `document_types` | `type_code` | `type_name` | EXISTS |
| `Role` | `roles` | `name` | `description` | EXISTS |
| `EntityType` | `entity_types` | `type_code` | `type_name` | EXISTS |
| `Jurisdiction` | `jurisdictions` (view) | `iso_code` | `name` | CREATED |
| `Attribute` | `attribute_dictionary` | `attr_id` | `attr_name` | CREATED |
| `ScreeningList` | `screening_lists` | `list_code` | `list_name` | CREATED |
| `Currency` | `currencies` | `iso_code` | `name` | CREATED |

### Migration: 018_lsp_lookup_tables.sql

Creates missing tables:
- `attribute_dictionary` - CBU, PERSON, COMPANY, DOCUMENT attributes
- `screening_lists` - OFAC, EU, UN, UK sanctions + PEP lists
- `currencies` - Major ISO currencies
- `jurisdictions` view - Aliases `master_jurisdictions`

---

## Part 3: Implemented Components

### 3.1 Server Core (`server.rs`)

```rust
pub struct DslLanguageServer {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
    symbols: Arc<RwLock<SymbolTable>>,
}

impl LanguageServer for DslLanguageServer {
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>>;
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>>;
    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>>;
    async fn did_open(&self, params: DidOpenTextDocumentParams);
    async fn did_change(&self, params: DidChangeTextDocumentParams);
    // ... etc
}
```

### 3.2 Completion Handler (`completion.rs`)

Provides context-aware completions:

1. **Verb names** - After `(`, suggests from `VERB_REGISTRY`
2. **Keywords** - After `:`, suggests from `VerbDef.args`
3. **Keyword values** - Based on `SemType`:
   - `Ref(RefType)` → Picklist from `SchemaCache`
   - `Enum(values)` → Fixed value list
   - `Symbol` → Session `@` symbols
4. **Symbol refs** - After `@`, suggests defined symbols

```rust
pub fn get_completions(doc: &DocumentState, position: Position, symbols: &SymbolTable) -> Vec<CompletionItem>;
```

### 3.3 Hover Handler (`hover.rs`)

Shows documentation on hover:
- **Verbs**: Description, arguments, examples
- **Keywords**: Type, required status, description
- **Symbols**: Definition location, verb that created it

### 3.4 Diagnostics Handler (`diagnostics.rs`)

Reports errors:
- `E001` - Unknown verb (with suggestions)
- `E002` - Unknown argument (with suggestions)
- `E003` - Missing required argument
- `E007` - Undefined symbol reference

### 3.5 Schema Cache (`cache.rs`)

Two modes:
1. `SchemaCache::with_defaults()` - Hardcoded test data
2. `SchemaCache::load_from_db(pool)` - Loads from PostgreSQL

```rust
impl SchemaCache {
    pub fn get_completions(&self, ref_type: &RefType) -> Vec<&LookupEntry>;
    pub fn exists(&self, ref_type: &RefType, code: &str) -> bool;
    pub fn suggest(&self, ref_type: &RefType, typo: &str) -> Vec<String>;
    
    #[cfg(feature = "database")]
    pub async fn load_from_db(pool: &PgPool) -> Result<Self, sqlx::Error>;
}
```

---

## Part 4: Crate Structure

```
rust/crates/dsl-lsp/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── server.rs            # LSP server implementation
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── document.rs      # Document state, parsed expressions
│   │   ├── symbols.rs       # Cross-document symbol table
│   │   └── context.rs       # Completion context detection
│   └── handlers/
│       ├── mod.rs
│       ├── completion.rs    # textDocument/completion
│       ├── hover.rs         # textDocument/hover
│       ├── diagnostics.rs   # Document analysis + errors
│       ├── goto_definition.rs
│       ├── signature.rs     # textDocument/signatureHelp
│       └── symbols.rs       # textDocument/documentSymbol
├── zed-extension/
│   ├── extension.json
│   └── languages/dsl/
│       ├── config.toml
│       ├── highlights.scm
│       └── indents.scm
└── tree-sitter-dsl/
    ├── grammar.js
    └── package.json
```

---

## Part 5: Dependencies

```toml
[dependencies]
tower-lsp = "0.20"
lsp-types = "0.95"
tokio = { version = "1", features = ["full", "sync"] }
ob-poc = { path = "../..", features = ["database"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
regex = "1"
```

---

## Part 6: IDE Experience

### 6.1 Verb Completion

User types: `(cbu`

```
┌─────────────────────────────────────────────────────────────────┐
│ cbu.ensure      [cbu] requires: :cbu-name                       │
│ cbu.create      [cbu] requires: :cbu-name                       │
│ cbu.attach-entity [cbu] requires: :entity-id, :role             │
│ cbu.detach-entity [cbu]                                         │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Keyword Completion

User types: `(cbu.ensure :`

```
┌─────────────────────────────────────────────────────────────────┐
│ :cbu-name      STRING (required)                                │
│ :jurisdiction  JURISDICTION_REF                                 │
│ :client-type   one of ["UCITS", "AIFM", ...]                    │
│ :as            SYMBOL (@name)                                   │
└─────────────────────────────────────────────────────────────────┘
```

### 6.3 Reference Completion (Option A)

User types: `(document.request :document-type "`

```
┌─────────────────────────────────────────────────────────────────┐
│ 📄 Certificate of Incorporation          CERT_OF_INCORP        │
│    Corporate                                                    │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Articles of Association               ARTICLES_OF_ASSOC     │
│    Corporate                                                    │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Passport                              PASSPORT              │
│    Identity                                                     │
└─────────────────────────────────────────────────────────────────┘
```

User selects "Certificate of Incorporation", DSL becomes:
```clojure
(document.request :document-type "CERT_OF_INCORP"
```

### 6.4 Symbol Completion

User types: `(cbu.attach-entity :entity-id @`

```
┌─────────────────────────────────────────────────────────────────┐
│ @company         EntityId from entity.create-limited-company    │
│ @person          EntityId from entity.create-proper-person      │
│ @fund            CbuId from cbu.ensure                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 7: Building and Running

### Build the LSP Server

```bash
cd rust
cargo build --release -p dsl-lsp
```

### Run Standalone

```bash
./target/release/dsl-lsp
```

### Zed Integration

1. Copy `zed-extension/` to `~/.config/zed/extensions/onboarding-dsl/`
2. Build tree-sitter grammar:
   ```bash
   cd tree-sitter-dsl
   npm install
   npm run build
   ```
3. Restart Zed

### VS Code Integration

Create `.vscode/settings.json`:
```json
{
  "dsl.serverPath": "./rust/target/release/dsl-lsp"
}
```

---

## Part 8: Testing

### Unit Tests

```bash
cargo test -p dsl-lsp
```

### Manual Testing

Create `test.dsl`:
```clojure
; Test file for LSP
(cbu.ensure :cbu-name "Test Fund" :jurisdiction "LU" :as @fund)

(entity.create-limited-company 
  :name "TestCo Ltd"
  :jurisdiction "GB"
  :as @company)

(cbu.attach-entity :cbu-id @fund :entity-id @company :role "InvestmentManager")
```

Open in IDE with LSP configured:
- Hover over `cbu.ensure` → See documentation
- Type `:role "` → Get role completions
- Type `@` → Get symbol completions
- Reference undefined symbol → See error

---

## Part 9: Future Enhancements

### Phase 2: Advanced Features
- [ ] Code actions (quick fixes)
- [ ] Rename symbol
- [ ] Find all references
- [ ] Workspace-wide analysis
- [ ] Incremental parsing for large files

### Phase 3: Database Integration
- [ ] Real-time schema cache refresh
- [ ] Connection to live database
- [ ] Attribute extraction suggestions based on document type

---

## Summary

| Component | Description |
|-----------|-------------|
| **Type System** | `SemType` with `Ref(RefType)` for lookup references |
| **Schema Cache** | `SchemaCache::load_from_db()` or `with_defaults()` |
| **Completion Flow** | Context detection → Semantic type → Query cache → Build picklist |
| **Option A Pattern** | Display human name, insert code, runtime resolves UUID |
| **LSP Protocol** | tower-lsp 0.20 with full capability support |
| **IDE Support** | Zed extension + tree-sitter grammar |

This delivers a full IDE experience where users see friendly names but the DSL remains portable with code identifiers.
