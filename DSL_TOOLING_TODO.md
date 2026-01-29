# DSL Tooling: Complete Implementation TODO

## Overview

This document consolidates all DSL tooling work into a single implementation plan:
- **Part A**: Parser/LSP correctness (spans, encoding, errors)
- **Part B**: Zed editor experience (tree-sitter queries, golden examples, agent grounding)

**Goal**: Make the DSL feel like a first-class language in Zed with correct LSP behavior,
rich syntax highlighting, outline navigation, run buttons, and agent-friendly annotations.

---

## Ground Rules / Invariants

1. All AST spans are absolute byte offsets into the original file
2. All LSP positions are encoding-aware (UTF-16 by default; negotiate if supported)
3. All node conversions use each node's own span (no parent span reused for children)
4. `:as` is a dedicated binding field, never a normal keyword argument
5. Completions/context scanning must ignore strings and comments and work across lines
6. Preceding `;;` comments become `@annotation` context for Zed Assistant

---

## Acceptance Criteria (Definition of Done)

### Parser/LSP Correctness
- [ ] Multi-statement file: span of statement #2 starts after statement #1
- [ ] Nested verb call: inner span is absolute, not relative to outer
- [ ] List items: each item has distinct span
- [ ] Map entries: keys and values have distinct spans
- [ ] UTF-16 with emoji: `(cbu.create :name "Test 🎉 Fund")` positions correct
- [ ] Parse errors show at actual location, not (0,0)
- [ ] Go-to-definition, references, rename all work correctly

### Zed Experience
- [ ] `.dsl`, `.obl`, `.onboard` files recognized as DSL (not Clojure)
- [ ] Rainbow brackets work for `()[]{}`
- [ ] Auto-indent feels Lisp-like
- [ ] Outline shows `domain.verb` with binding name
- [ ] Preceding `;;` comments appear as `@annotation` for Assistant
- [ ] Run buttons appear beside top-level forms
- [ ] Golden examples parse in both tree-sitter and NOM

---

# PART A: Parser/LSP Correctness

## Phase 1: Foundation Fixes (Critical)

### 1.1 Integrate `nom_locate` for Absolute Span Tracking

**File**: `rust/crates/dsl-core/src/parser.rs`

**Problem**: Top-level `verb_call()` sets `start_offset = 0`, making spans relative 
to the slice where parsing started, not the whole file.

**Fix**:
```rust
// Add to Cargo.toml
nom_locate = "4"

// In parser.rs
use nom_locate::LocatedSpan;
pub type Span<'a> = LocatedSpan<&'a str>;

// Update all parser functions to use Span<'a> instead of &'a str
// Use input.location_offset() to get absolute byte offset
fn verb_call(input: Span) -> IResult<Span, VerbCall> {
    let start = input.location_offset();
    // ... existing parse logic ...
    let end = remaining.location_offset();
    Ok((remaining, VerbCall { 
        ..., 
        span: AstSpan::new(start, end) 
    }))
}
```

**Affected functions**: `parse_program()`, `program()`, `statement()`, `comment()`,
`verb_call()`, `argument_with_span()`, `value_parser_with_span()`, `list_literal()`,
`map_literal()`, all literal parsers.

---

### 1.2 Implement UTF-16 Position Encoding

**Files**: 
- `rust/crates/dsl-lsp/src/encoding.rs` (new)
- `rust/crates/dsl-lsp/src/lib.rs` (initialize handler)

**Problem**: AST spans are byte offsets, LSP uses UTF-16 code units.

**Fix**: Create encoding module:

```rust
// rust/crates/dsl-lsp/src/encoding.rs
use tower_lsp::lsp_types::Position;

#[derive(Clone, Copy, Default)]
pub enum PositionEncoding {
    #[default]
    Utf16,
    Utf8,
}

pub fn offset_to_position(text: &str, offset: usize, enc: PositionEncoding) -> Position {
    let mut line = 0u32;
    let mut line_start_offset = 0usize;
    
    for (i, c) in text.char_indices() {
        if i >= offset { break; }
        if c == '\n' {
            line += 1;
            line_start_offset = i + 1;
        }
    }
    
    let line_text = &text[line_start_offset..offset.min(text.len())];
    let character = match enc {
        PositionEncoding::Utf16 => line_text.encode_utf16().count() as u32,
        PositionEncoding::Utf8 => line_text.len() as u32,
    };
    
    Position { line, character }
}

pub fn position_to_offset(text: &str, pos: Position, enc: PositionEncoding) -> Option<usize>;
pub fn span_to_range(text: &str, start: usize, end: usize, enc: PositionEncoding) -> Range;
```

Negotiate encoding in `initialize()` and store in server state.

---

### 1.3 Fix Comment Rendering Bug

**File**: `rust/crates/dsl-core/src/ast.rs`

**Problem**: Renders `;` but grammar requires `;;`.

**Fix**:
```rust
// Change:
Statement::Comment(c) => format!("; {}", c),
// To:
Statement::Comment(c) => format!(";; {}", c),
```

---

## Phase 2: AST Span Completeness

### 2.1 Add Spans to All AST Nodes

**File**: `rust/crates/dsl-core/src/ast.rs`

**Problem**: Literals don't have spans; adapter reuses parent span for children.

**Fix**: Every `AstNode` variant must have a span:

```rust
pub enum AstNode {
    Literal { value: Literal, span: Span },
    SymbolRef { name: String, span: Span },
    EntityRef { ..., span: Span, ... },
    List { items: Vec<AstNode>, span: Span },
    Map { entries: Vec<MapEntry>, span: Span },
    Nested(Box<VerbCall>),
}

pub struct MapEntry {
    pub key: String,
    pub key_span: Span,
    pub value: AstNode,
}

impl AstNode {
    pub fn span(&self) -> Span {
        match self {
            AstNode::Literal { span, .. } => *span,
            AstNode::SymbolRef { span, .. } => *span,
            AstNode::EntityRef { span, .. } => *span,
            AstNode::List { span, .. } => *span,
            AstNode::Map { span, .. } => *span,
            AstNode::Nested(vc) => vc.span,
        }
    }
}
```

### 2.2 Update v2_adapter to Use Node Spans

**File**: `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`

**Rule**: `convert_node(node)` uses `node.span()` always—never reuse parent span.

---

## Phase 3: Error Reporting

### 3.1 ParseError with Byte Offsets

**File**: `rust/crates/dsl-core/src/parser.rs`

```rust
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub start: Option<usize>,  // byte offset
    pub end: Option<usize>,
}

pub fn parse_program(input: &str) -> Result<Program, ParseError>;
```

### 3.2 Diagnostics via Encoding Module

**File**: `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`

Use `span_to_range()` for all diagnostic ranges. No more `Range::default()`.

---

## Phase 4: Completion Context Fix

### 4.1 Multiline Context Detection

**File**: `rust/crates/dsl-lsp/src/analysis/context.rs`

**Problem**: Only scans current line, breaks for multiline forms.

**Fix**: Convert position → offset, scan `&doc.text[..offset]` (full prefix).

### 4.2 Ignore Strings and Comments

Replace naive `rfind('@')` with a scanner that tracks:
- `in_string: bool` (handle escapes)
- `in_comment: bool` (set on `;;`, reset on `\n`)
- `paren_depth`, `bracket_depth`, `brace_depth`

Prevent `"email@domain.com"` and `;; @fund` from triggering completion.

---

## Phase 5: LSP Semantic Features

### 5.1 textDocument/hover

**File**: `rust/crates/dsl-lsp/src/handlers/hover.rs`

- Hover on verb: show description + args from YAML registry
- Hover on symbol: show type + definition location

### 5.2 textDocument/documentSymbol

**File**: `rust/crates/dsl-lsp/src/handlers/document_symbol.rs`

- Each verb call → outline item
- Include binding name if present
- Use `call.binding` field, not searching args for `:as`

### 5.3 Go-to-definition / References / Rename

**Files**: `handlers/definition.rs`, `handlers/references.rs`, `handlers/rename.rs`

Build per-document symbol index:
- `decls: HashMap<String, Location>` — from `:as @sym`
- `refs: HashMap<String, Vec<Location>>` — from `@sym` usages

---

# PART B: Zed Editor Experience

## Phase 6: Tree-sitter Grammar Enhancement

### 6.1 Ensure `:as` is Dedicated Node

**File**: `rust/crates/dsl-lsp/tree-sitter-dsl/grammar.js`

**Critical**: `:as @x` must parse as `as_binding` node, not `keyword + symbol_ref`.

```javascript
// Use negative lookahead to exclude :as from normal keywords
keyword_except_as: ($) => token(seq(":", /(?!as\b)[a-zA-Z_][a-zA-Z0-9_\-]*/)),

keyword_arg: ($) => seq($.keyword_except_as, $._value),

as_binding: ($) => seq(":as", $.symbol_ref),

verb_call: ($) => seq(
    "(",
    optional($.verb_name),
    repeat($._expression),
    optional($.as_binding),
    ")"
),
```

After changes: `npx tree-sitter generate`

---

## Phase 7: Zed Extension Structure

### 7.1 Create Extension Directory

**Path**: `rust/crates/dsl-lsp/zed-extension/`

```
zed-extension/
├── extension.toml
├── languages/
│   └── dsl/
│       ├── config.toml
│       ├── highlights.scm
│       ├── brackets.scm
│       ├── indents.scm
│       ├── outline.scm
│       ├── textobjects.scm
│       ├── overrides.scm
│       └── runnables.scm
└── snippets/
    └── dsl.json
```

### 7.2 extension.toml

```toml
[package]
id = "ob-poc-dsl"
name = "OB-POC DSL"
version = "0.1.0"
schema_version = 1

[grammars.dsl]
repository = "https://github.com/your-org/ob-poc"
rev = "main"
path = "rust/crates/dsl-lsp/tree-sitter-dsl"

[language_servers.dsl-lsp]
name = "DSL Language Server"
languages = ["DSL"]
```

### 7.3 config.toml

```toml
name = "DSL"
grammar = "dsl"
path_suffixes = ["dsl", "obl", "onboard"]
line_comments = [";;"]
word_characters = ["@", ":", "-", "_", "."]
tab_size = 2
```

---

## Phase 8: Tree-sitter Query Files

### 8.1 highlights.scm

```scheme
(comment) @comment
(verb_name) @function
(keyword) @property
(as_binding ":as" @keyword.special)
(symbol_ref) @variable
(as_binding (symbol_ref) @variable.special)
(string) @string
(number) @number
(boolean) @constant.builtin
(null_literal) @constant.builtin
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
```

### 8.2 brackets.scm

```scheme
("(" @open) (")" @close)
("[" @open) ("]" @close)
("{" @open) ("}" @close)
```

### 8.3 indents.scm

```scheme
(verb_call "(" @indent ")" @end)
(array "[" @indent "]" @end)
(map "{" @indent "}" @end)
```

### 8.4 outline.scm (Critical for Agent Integration)

```scheme
;; Each verb call is an outline item
(verb_call (verb_name) @name) @item

;; Show binding in outline
(verb_call
  (verb_name) @name
  (as_binding (symbol_ref) @context.extra)) @item

;; Preceding comments become @annotation for Assistant
(comment)+ @annotation . (verb_call) @item
```

### 8.5 textobjects.scm

```scheme
(verb_call) @function.outer
(array) @list.outer
(map) @block.outer
(comment) @comment.outer
```

### 8.6 runnables.scm

```scheme
(verb_call
  (verb_name) @run
  (#set! tag "dsl-form"))
```

---

## Phase 9: Snippets

**File**: `zed-extension/snippets/dsl.json`

```json
{
  "CBU Create": {
    "prefix": ["cbu", "cbu.ensure"],
    "body": [
      ";; intent: ${1:Create custody banking unit}",
      "(cbu.ensure",
      "  :name \"${2:Fund Name}\"",
      "  :jurisdiction \"${3|LU,IE,US,GB|}\"",
      "  :as @${4:cbu})"
    ]
  },
  "Entity Person": {
    "prefix": ["person"],
    "body": [
      ";; intent: ${1:Create natural person}",
      "(entity.create-proper-person",
      "  :first-name \"${2:First}\"",
      "  :last-name \"${3:Last}\"",
      "  :as @${4:person})"
    ]
  },
  "Intent Block": {
    "prefix": [";;", "intent"],
    "body": [
      ";; intent: ${1:What this accomplishes}",
      ";; macro: ${2:operator.verb-name}"
    ]
  }
}
```

---

## Phase 10: Tasks (Runnables Integration)

**File**: `.zed/tasks.json`

```json
{
  "tasks": [
    {
      "label": "DSL: Validate Form",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "validate", "--file", "$ZED_FILE"],
      "tags": ["dsl-form"]
    },
    {
      "label": "DSL: Expand Macro",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "expand", "--file", "$ZED_FILE"],
      "tags": ["dsl-form"]
    },
    {
      "label": "DSL: Format File",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "fmt", "$ZED_FILE"]
    }
  ]
}
```

---

## Phase 11: Golden Examples Suite

**Path**: `docs/dsl/golden/`

### Required Files

| File | Purpose |
|------|---------|
| `00-syntax-tour.dsl` | All syntax constructs |
| `01-cbu-create.dsl` | Hello world CBU |
| `02-roles-and-links.dsl` | Entity + role assignment |
| `03-kyc-case-sheet.dsl` | Case with entity list |
| `04-ubo-mini-graph.dsl` | Ownership chain |
| `05-otc-isda-csa.dsl` | Realistic OTC onboarding |
| `06-macro-v2-roundtrip.dsl` | Macro expansion example |
| `90-error-fixtures.dsl` | Intentional parse errors |

### Mandatory Annotation Pattern

Every top-level form must have:

```clojure
;; intent: <1 sentence business goal>
;; macro: <operator.verb-name or primitive>
(verb.call :args ... :as @binding)
```

This is how Zed Assistant gets grounded context via `@annotation`.

---

## Phase 12: Documentation

### 12.1 docs/DSL_STYLE_GUIDE.md

- Formatting rules (2-space indent, one arg per line when multiline)
- `:as` always last
- Annotation blocks required for reviewable sheets
- Symbol naming conventions (`@fund`, `@manco`, `@john`)

### 12.2 docs/AGENT_RULES.md

- Code generation loop (select verb → slot-fill → validate → fix → stop)
- Annotation preservation rules
- Error recovery (max 3 fix attempts)

### 12.3 docs/ZED_SETUP.md

- Extension installation (local dev vs published)
- LSP configuration
- File associations
- Keyboard shortcuts
- Troubleshooting

---

## Phase 13: Tests

### 13.1 Tree-sitter Parse Tests

**File**: `tree-sitter-dsl/test/corpus/golden.txt`

Verify:
- Every top-level form is `verb_call`
- `:as @x` becomes `as_binding` (not `keyword + symbol_ref`)
- Map entries are `map_entry` pairs

### 13.2 Golden Example Validation

**File**: `tests/golden_validation.rs`

```rust
#[test]
fn all_golden_examples_parse_with_nom() {
    // Parse each docs/dsl/golden/*.dsl (skip 90-*)
    // Assert Ok
}

#[test]
fn golden_examples_have_annotations() {
    // Check for ";; intent:" in non-trivial examples
}
```

---

## Dependency Order Summary

```
PART A: Parser/LSP
Phase 1.1 (nom_locate)
    ↓
Phase 1.2 (UTF-16 encoding)
    ↓
Phase 2.1 (AST spans) ←── Phase 1.3 (comment bug)
    ↓
Phase 2.2 (adapter update)
    ↓
Phase 3.* (error reporting)
    ↓
Phase 4.* (completion context)
    ↓
Phase 5.* (LSP features)

PART B: Zed Experience
Phase 6.1 (grammar :as fix) ←── Required before queries work
    ↓
Phase 7.* (extension structure)
    ↓
Phase 8.* (query files)
    ↓
Phase 9-10 (snippets + tasks)
    ↓
Phase 11 (golden examples)
    ↓
Phase 12-13 (docs + tests)
```

---

## Files Modified/Created Summary

### Part A (Parser/LSP)

| File | Changes |
|------|---------|
| `dsl-core/Cargo.toml` | Add `nom_locate = "4"` |
| `dsl-core/src/parser.rs` | nom_locate, absolute spans, ParseError |
| `dsl-core/src/ast.rs` | Spans on all nodes, comment fix |
| `dsl-lsp/src/encoding.rs` | New: position encoding |
| `dsl-lsp/src/lib.rs` | Negotiate encoding |
| `dsl-lsp/src/analysis/*.rs` | Use encoding, node spans |
| `dsl-lsp/src/handlers/*.rs` | hover, documentSymbol, def, refs, rename |

### Part B (Zed Experience)

| File | Changes |
|------|---------|
| `tree-sitter-dsl/grammar.js` | `:as` as dedicated node |
| `zed-extension/extension.toml` | Extension manifest |
| `zed-extension/languages/dsl/config.toml` | Language config |
| `zed-extension/languages/dsl/*.scm` | 7 query files |
| `zed-extension/snippets/dsl.json` | Code snippets |
| `.zed/tasks.json` | Repository tasks |
| `docs/dsl/golden/*.dsl` | 8 golden examples |
| `docs/DSL_STYLE_GUIDE.md` | Style guide |
| `docs/AGENT_RULES.md` | Agent rules |
| `docs/ZED_SETUP.md` | Setup instructions |

---

## Notes on Entity Resolution

Entity resolution (`"Apex Fund"` → UUID) is **not** an LSP concern:
- **Tier 1**: Syntax (parser) — LSP validates
- **Tier 2**: Schema (YAML registry) — LSP validates
- **Tier 3**: Resolution (EntityGateway) — execution-time only

LSP can assist with completions but cannot guarantee resolution will succeed.
This separation is correct for regulated finance audit trails.
