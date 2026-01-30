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

### Incremental Edit Stability
- [ ] Rename `@fund` → `@fund2`: highlight updates immediately (no reload)
- [ ] Rename `@fund` → `@fund2`: outline updates immediately
- [ ] Rename `@fund` → `@fund2`: run button remains on form
- [ ] Partial edit `(cbu.ensure :name "Apex|` still highlights correctly outside string

---

## Key Implementation Risks + Mitigations

These are the "80% debugging time" traps. Address them explicitly or they will bite.

### Risk 1: Completion Context Scanner (Nested S-expressions + Partial Edits)

**Where it bites**: Phase 4.2 "ignore strings/comments" is necessary but not sufficient.

**Failure modes**:
- Complete at `@` inside nested calls → engine misidentifies whether you're completing 
  a symbol ref, keyword value, or verb head
- Think you're inside string/comment when you aren't (escaped quotes, incomplete strings)
- Cannot determine current `verb_call` because of nested `()` and `[]`/`{}`

**Mitigation**: See expanded Phase 4.2 requirements below.

### Risk 2: Incremental Parsing (Tree-sitter vs Zed)

**Where it bites**: Tree-sitter is incremental, but grammar must support stable parsing 
during edits. If you edit `:as @binding`, you want immediate highlight/outline updates 
without reload.

**Failure modes**:
- `:as` parses as `keyword_arg` instead of `as_binding` → stale highlighting
- Half-typed structures cause tree to collapse → outline disappears while typing
- Grammar ambiguity causes tree-sitter to keep old parse shape

**Mitigation**: See expanded Phase 6.1 requirements below.

### Risk 3: Node Name Mismatch (Tree-sitter vs NOM AST)

**Where it bites**: Different names for same concept makes debugging brutal.

**Mitigation**: Enforce node name contract:
| Concept | Tree-sitter Node | NOM AST Type |
|---------|------------------|--------------|
| Verb call | `verb_call` | `VerbCall` |
| Verb name | `verb_name` | `domain + verb` |
| Keyword arg | `keyword_arg` | `Argument` |
| Binding | `as_binding` | `VerbCall.binding` |
| Symbol ref | `symbol_ref` | `AstNode::SymbolRef` |
| Map entry | `map_entry` | `MapEntry` |

### Risk 4: No Logging = Blind Debugging

**Mitigation**: Add debug mode to LSP that logs:
- Completion context classification (`InsideString`, `InsideComment`, `AtSymbolRef`, 
  `AfterKeywordAwaitingValue`, `AtVerbHead`)
- Current call head extraction result
- Cursor offset/position conversions

Add dev task: "DSL: Dump Tree-sitter node at cursor"

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

### 4.2 Ignore Strings and Comments (Full Scanner Implementation)

**File**: `rust/crates/dsl-lsp/src/analysis/context.rs`

Replace naive `rfind('@')` with a proper prefix scanner.

#### 4.2.1 Scanner State

Track these while scanning `text[..offset]`:

```rust
struct PrefixScanner {
    in_string: bool,
    escape_next: bool,
    in_comment: bool,
    paren_depth: i32,
    bracket_depth: i32,
    brace_depth: i32,
    // Stack of absolute byte offsets for open parens
    paren_stack: Vec<usize>,
    // Most recent keyword token (start, end) outside string/comment
    last_keyword: Option<(usize, usize)>,
    // Most recent @ outside string/comment
    last_at: Option<usize>,
}
```

**Rules**:
- `in_comment` starts on `;;` when not in string, ends at `\n`
- `in_string` toggles on `"` unless `escape_next`
- `escape_next` set after `\` in string, cleared after next char
- Push to `paren_stack` on `(`, pop on `)` (when not in string/comment)

#### 4.2.2 Determine "Current Call Head"

Use the paren stack to find the current verb call:

```rust
fn current_call_context(&self, text: &str) -> Option<CallContext> {
    // Find deepest open paren not inside brackets/braces
    let call_start = self.paren_stack.last()?;
    
    // Parse slice from open paren to cursor with tolerant parser
    let slice = &text[*call_start..];
    
    // Extract:
    // - head: Option<String> (domain.verb if present)
    // - used_keywords: Vec<String> (for duplicate avoidance)
    // - awaiting_value: bool (last keyword has no value yet)
}
```

#### 4.2.3 Completion Trigger Rules

| Context | Trigger | Result |
|---------|---------|--------|
| `@` outside string/comment, followed by ident chars | `@` completion | Symbol suggestions |
| After `:keyword` with no value yet | Value completion | Type-appropriate values |
| After `(` at call head position | Verb completion | Verb suggestions |
| Inside `"..."` | NO completion | — |
| Inside `;; ...` | NO completion | — |
| `"email@domain.com"` | NO completion | — |

#### 4.2.4 Hard Test Fixtures

**Must pass**:

```rust
#[test]
fn test_nested_completion() {
    // Cursor at | after @
    let doc = "(outer.call :x (inner.call :ref @|))";
    let ctx = detect_completion_context(&doc, pos(0, 35));
    
    // Should identify inner.call as current verb
    assert!(matches!(ctx, CompletionContext::SymbolRef { 
        verb_name: Some(v), .. 
    } if v == "inner.call"));
}

#[test]
fn test_keyword_awaiting_value() {
    let doc = "(cbu.ensure :name |)";
    let ctx = detect_completion_context(&doc, pos(0, 18));
    
    assert!(matches!(ctx, CompletionContext::KeywordValue { 
        keyword, .. 
    } if keyword == "name"));
}

#[test]
fn test_no_completion_in_string() {
    let doc = r#"(test.echo :message "email@domain.com|")"#;
    let ctx = detect_completion_context(&doc, pos(0, 37));
    
    // Should NOT trigger @ completion
    assert!(matches!(ctx, CompletionContext::None) || 
            matches!(ctx, CompletionContext::KeywordValue { in_string: true, .. }));
}

#[test]
fn test_no_completion_in_comment() {
    let doc = ";; comment mentioning @fund|";
    let ctx = detect_completion_context(&doc, pos(0, 27));
    
    assert!(matches!(ctx, CompletionContext::None));
}

#[test]
fn test_incomplete_string_tolerance() {
    // Incomplete string - completion should still work OUTSIDE the string
    let doc = r#"(cbu.ensure :name "Apex| :as @fund)"#;
    // Cursor after "Apex is inside string - no completion
    // But the parser shouldn't crash
}

#[test]  
fn test_multiple_nested_parens() {
    let doc = "(a.b :x (c.d :y (e.f :z @|)))";
    let ctx = detect_completion_context(&doc, pos(0, 24));
    
    // Current verb should be e.f (deepest)
    assert!(matches!(ctx, CompletionContext::SymbolRef {
        verb_name: Some(v), ..
    } if v == "e.f"));
}
```

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

### 5.4 Debug Logging Mode

**File**: `rust/crates/dsl-lsp/src/lib.rs`

Add debug mode (enabled via init option or env var) that logs:
- Completion context classification: `InsideString`, `InsideComment`, `AtSymbolRef`, 
  `AfterKeywordAwaitingValue`, `AtVerbHead`
- Current call head extraction result
- Cursor offset/position conversions
- Parse timing

```rust
// In completion handler
if self.debug_mode {
    tracing::debug!(
        "completion context: {:?}, verb={:?}, offset={}, encoding={:?}",
        context_kind, current_verb, offset, self.position_encoding
    );
}
```

Without this, debugging completion issues is brutal.

---

# PART B: Zed Editor Experience

## Phase 6: Tree-sitter Grammar Enhancement

### 6.1 Ensure `:as` is Dedicated Node (Non-negotiable Invariant)

**File**: `rust/crates/dsl-lsp/tree-sitter-dsl/grammar.js`

**Critical**: `:as @x` must parse as `as_binding` node, not `keyword + symbol_ref`.
This is the #1 cause of stale highlights during edits.

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

### 6.2 Edit-Tolerant Grammar Rules

Make the grammar resilient to half-typed structures. This keeps the syntax tree
stable while typing, which keeps highlights/outline/runnables stable.

```javascript
// Allow keyword_arg with missing value (shows as ERROR but tree stays stable)
keyword_arg: ($) => seq($.keyword_except_as, optional($._value)),

// Allow as_binding with missing symbol_ref temporarily  
as_binding: ($) => seq(":as", optional($.symbol_ref)),

// Verb call tolerates missing closing paren (tree-sitter handles via ERROR)
verb_call: ($) => seq(
    "(",
    optional($.verb_name),
    repeat($._expression),
    optional($.as_binding),
    optional(")")  // Makes partial edits more stable
),
```

### 6.3 Tree-sitter Corpus Tests for Incremental Edits

**File**: `tree-sitter-dsl/test/corpus/incremental.txt`

Assert `:as` becomes `as_binding` even during typing sequences:

```
================================================================================
Binding with complete symbol
================================================================================

(cbu.ensure :name "Test" :as @fund)

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg (keyword) (string))
    (as_binding (symbol_ref))))

================================================================================
Binding without symbol yet (mid-typing @)
================================================================================

(cbu.ensure :name "Test" :as @)

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg (keyword) (string))
    (as_binding (symbol_ref))))

================================================================================
Keyword :as without symbol (just typed :as)
================================================================================

(cbu.ensure :name "Test" :as)

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg (keyword) (string))
    (as_binding)))

================================================================================
Partial :a typing (should NOT be as_binding yet)
================================================================================

(cbu.ensure :name "Test" :a)

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg (keyword) (string))
    (keyword_arg (keyword))))
```

### 6.4 Query Robustness

In `highlights.scm` and `outline.scm`, prefer capturing by node name rather than
matching literal token sequences. Incremental parsing may produce ERROR nodes.

```scheme
;; Good: capture node
(as_binding) @keyword.special

;; Also highlight if symbol missing (mid-edit)
(as_binding ":as" @keyword.special)
(as_binding (symbol_ref) @variable.special)

;; Outline: show item even without binding symbol
(verb_call (verb_name) @name) @item
(verb_call 
  (verb_name) @name
  (as_binding (symbol_ref)? @context.extra)) @item
```

### 6.5 Incremental Refresh Acceptance Tests

Add to acceptance criteria:
- [ ] Rename `@fund` → `@fund2`: highlight updates immediately (no reload)
- [ ] Rename `@fund` → `@fund2`: outline updates immediately
- [ ] Rename `@fund` → `@fund2`: run button remains on form
- [ ] Type `:as @` then identifier: binding highlights progressively
- [ ] Partial edit `(cbu.ensure :name "Apex|` doesn't break highlighting outside string

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
│       ├── redactions.scm      # Optional: PII redaction for demos
│       └── runnables.scm
└── snippets/
    └── dsl.json
```

### 7.2 extension.toml

**Note**: Zed uses top-level keys, not `[package]`. Use commit SHA for `rev` (not branch name).
For local dev, use `repository = "file:///path/to/repo"`.

```toml
id = "ob-poc-dsl"
name = "OB-POC DSL"
description = "Language support for the OB-POC KYC/AML onboarding DSL"
version = "0.1.0"
schema_version = 1
authors = ["BNY Mellon Enterprise Onboarding Team"]
repository = "https://github.com/your-org/ob-poc"

[grammars.dsl]
repository = "https://github.com/your-org/ob-poc"
# For dev: repository = "file:///path/to/ob-poc"
rev = "abc123def456"  # Must be commit SHA, not branch name
path = "rust/crates/dsl-lsp/tree-sitter-dsl"

# NOTE: For LSP auto-launch, choose ONE path:
#
# PATH A (fast): Extension provides grammar/queries only.
#   Configure LSP in Zed settings.json:
#   "lsp": { "dsl-lsp": { "binary": { "path": "/path/to/dsl-lsp" } } }
#
# PATH B (full): Extension implements language_server_command in Rust.
#   Requires extension Rust code - see Zed extension docs.
#
# For initial implementation, use Path A.
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

**Note**: Zed expects paired `@open`/`@close` in a single pattern for rainbow brackets.

```scheme
;; Parentheses (S-expressions)
("(" @open ")" @close)

;; Square brackets (arrays/lists)
("[" @open "]" @close)

;; Curly braces (maps)
("{" @open "}" @close)

;; Exclude quotes from rainbow coloring
(string ("\"" @open "\"" @close) (#set! rainbow.exclude))
```

### 8.3 indents.scm

**Note**: Zed expects `(node CLOSE @end) @indent` pattern.

```scheme
;; Verb calls indent, dedent on close paren
(verb_call ")" @end) @indent

;; Arrays
(array "]" @end) @indent

;; Maps
(map "}" @end) @indent
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

**Note**: Zed expects `@function.around`/`@function.inside`, `@class.around`/`@class.inside`, 
`@comment.around` (not `@function.outer`/`@function.inner`).

```scheme
;; Verb calls as "functions"
(verb_call
  "("
  (_)* @function.inside
  ")") @function.around

;; Maps as "class" (for structural selection)
(map
  "{"
  (_)* @class.inside
  "}") @class.around

;; Arrays also as "class"
(array
  "["
  (_)* @class.inside
  "]") @class.around

;; Comments
(comment)+ @comment.around
```

### 8.6 overrides.scm

**Note**: Use `@comment.inclusive` for line comments so scope reaches newline.

```scheme
;; Inside strings: disable certain completions
(string) @string

;; Inside comments: inclusive so scope reaches newline
(comment) @comment.inclusive
```

### 8.7 redactions.scm (Optional - Banking Demo Friendly)

Zed supports redacting PII during collaboration/screen share.

```scheme
;; Redact values for sensitive keywords
(keyword_arg
  (keyword) @_kw
  (string) @redact
  (#match? @_kw ":(passport-number|tax-id|ssn|dob|date-of-birth|bank-account)"))
```

### 8.8 runnables.scm

**Note**: Zed exposes non-underscore-prefixed captures as `ZED_CUSTOM_<name>` env vars.
Use `(#set! tag ...)` to bind tasks by tag.

```scheme
;; Basic form: run button on verb name
(
  (verb_call
    (verb_name) @run @verb)
  (#set! tag dsl-form)
)

;; Form with binding: also capture the symbol
(
  (verb_call
    (verb_name) @run @verb
    (as_binding
      (symbol_ref) @binding))
  (#set! tag dsl-form-with-binding)
)
```

**Result**: Tasks can access `$ZED_CUSTOM_verb` and `$ZED_CUSTOM_binding`.

---

## Phase 9: Snippets

**File**: `zed-extension/snippets/dsl.json`

**Note**: Zed only uses the first prefix in a list. Use single strings or separate entries.

```json
{
  "CBU Create": {
    "prefix": "cbu",
    "body": [
      ";; intent: ${1:Create custody banking unit}",
      "(cbu.ensure",
      "  :name \"${2:Fund Name}\"",
      "  :jurisdiction \"${3|LU,IE,US,GB|}\"",
      "  :as @${4:cbu})"
    ]
  },
  "CBU Ensure": {
    "prefix": "cbu.ensure",
    "body": [
      ";; intent: ${1:Create custody banking unit}",
      "(cbu.ensure",
      "  :name \"${2:Fund Name}\"",
      "  :jurisdiction \"${3|LU,IE,US,GB|}\"",
      "  :as @${4:cbu})"
    ]
  },
  "Entity Person": {
    "prefix": "person",
    "body": [
      ";; intent: ${1:Create natural person}",
      "(entity.create-proper-person",
      "  :first-name \"${2:First}\"",
      "  :last-name \"${3:Last}\"",
      "  :as @${4:person})"
    ]
  },
  "Intent Block": {
    "prefix": "intent",
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

**Note**: Zed provides `$ZED_ROW` and `$ZED_COLUMN` for cursor position.
Tasks bound by tag use captures from runnables.scm.

```json
[
  {
    "label": "DSL: Validate Form",
    "command": "cargo",
    "args": ["run", "-p", "dsl-cli", "--", "validate", "--file", "$ZED_FILE", "--row", "$ZED_ROW", "--column", "$ZED_COLUMN"],
    "tags": ["dsl-form", "dsl-form-with-binding"],
    "reveal": "always"
  },
  {
    "label": "DSL: Expand Macro",
    "command": "cargo",
    "args": ["run", "-p", "dsl-cli", "--", "expand", "--file", "$ZED_FILE", "--form", "$ZED_CUSTOM_verb"],
    "tags": ["dsl-form", "dsl-form-with-binding"],
    "reveal": "always"
  },
  {
    "label": "DSL: Format File",
    "command": "cargo",
    "args": ["run", "-p", "dsl-cli", "--", "fmt", "$ZED_FILE"],
    "reveal": "never"
  },
  {
    "label": "DSL: Dump Tree-sitter Node at Cursor",
    "command": "npx",
    "args": ["tree-sitter", "parse", "$ZED_FILE"],
    "reveal": "always"
  },
  {
    "label": "DSL: Validate All Golden Examples",
    "command": "cargo",
    "args": ["run", "-p", "dsl-cli", "--", "validate", "--dir", "docs/dsl/golden/"],
    "reveal": "always"
  }
]
```

**CLI interface required**: `dsl-cli validate --file FILE --row N --column N`

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
Phase 4.* (completion context + scanner)
    ↓
Phase 5.* (LSP features + debug logging)

PART B: Zed Experience
Phase 6.1 (grammar :as fix) ←── Required before queries work
    ↓
Phase 6.2-6.5 (edit tolerance + corpus tests + query robustness)
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
| `dsl-lsp/src/lib.rs` | Negotiate encoding, debug mode flag |
| `dsl-lsp/src/analysis/context.rs` | Full prefix scanner with nesting |
| `dsl-lsp/src/analysis/*.rs` | Use encoding, node spans |
| `dsl-lsp/src/handlers/*.rs` | hover, documentSymbol, def, refs, rename |
| `dsl-lsp/tests/completion_context.rs` | Scanner test fixtures |

### Part B (Zed Experience)

| File | Changes |
|------|---------|
| `tree-sitter-dsl/grammar.js` | `:as` as dedicated node, edit-tolerant rules |
| `tree-sitter-dsl/test/corpus/incremental.txt` | Incremental edit tests |
| `zed-extension/extension.toml` | Extension manifest (top-level keys, SHA rev) |
| `zed-extension/languages/dsl/config.toml` | Language config |
| `zed-extension/languages/dsl/*.scm` | 8 query files (with correct Zed syntax) |
| `zed-extension/snippets/dsl.json` | Code snippets (single prefix strings) |
| `.zed/tasks.json` | Repository tasks (JSON array, row/col params) |
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
