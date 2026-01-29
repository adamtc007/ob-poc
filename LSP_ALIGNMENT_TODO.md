# DSL-LSP Alignment Fix: Implementation TODO

## Overview

This document consolidates all identified issues and enhancements from peer review.
The fixes are ordered by dependency and priority - later items depend on earlier ones.

**Goal**: Make the LSP feel "correct" in Zed with proper positioning, spans, and 
semantic features while maintaining the single-source-of-truth architecture 
(NOM parser shared by LSP and Agent).

---

## Phase 1: Foundation Fixes (Critical - Blocks Everything)

### 1.1 Integrate `nom_locate` for Absolute Span Tracking

**File**: `rust/crates/dsl-core/src/parser.rs`

**Problem**: Top-level `verb_call()` sets `start_offset = 0`, making spans relative 
to the slice where parsing started, not the whole file. This breaks all LSP features 
that depend on absolute positions.

**Fix**:
```rust
// Add to Cargo.toml
nom_locate = "4"

// In parser.rs
use nom_locate::LocatedSpan;

pub type Span<'a> = LocatedSpan<&'a str>;

// Update all parser functions to use Span<'a> instead of &'a str
// Use input.location_offset() to get absolute byte offset
// Example:
fn verb_call(input: Span) -> IResult<Span, VerbCall> {
    let start = input.location_offset();
    // ... existing parse logic, but with Span type ...
    let end = remaining.location_offset();
    Ok((remaining, VerbCall { 
        ..., 
        span: AstSpan::new(start, end) 
    }))
}
```

**Affected functions** (all need Span type):
- `parse_program()`
- `program()`
- `statement()`
- `comment()`
- `verb_call()`
- `verb_call_with_span()`
- `argument_with_span()`
- `value_parser_with_span()`
- `list_literal()`
- `map_literal()`
- All literal parsers

**Tests to verify**:
- Span for second statement in multi-statement program starts > 0
- Nested verb call spans are absolute, not relative to parent

---

### 1.2 Implement UTF-16 Position Encoding

**Files**: 
- `rust/crates/dsl-lsp/src/analysis/document.rs`
- `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`
- `rust/crates/dsl-lsp/src/lib.rs` (initialize handler)

**Problem**: AST spans are byte offsets, but `offset_to_position()` walks 
`text.chars().enumerate()` (char index). LSP defaults to UTF-16 code units. 
Zed explicitly uses UTF-16.

**Fix**:

1. Create a dedicated encoding module:
```rust
// rust/crates/dsl-lsp/src/encoding.rs

use tower_lsp::lsp_types::Position;

/// Position encoding mode (negotiated at initialize)
#[derive(Clone, Copy, Default)]
pub enum PositionEncoding {
    #[default]
    Utf16,
    Utf8,
}

/// Convert byte offset to LSP Position (line, character)
pub fn offset_to_position(text: &str, offset: usize, encoding: PositionEncoding) -> Position {
    let mut line = 0u32;
    let mut line_start_offset = 0usize;
    
    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            line_start_offset = i + 1;
        }
    }
    
    let line_text = &text[line_start_offset..offset.min(text.len())];
    let character = match encoding {
        PositionEncoding::Utf16 => line_text.encode_utf16().count() as u32,
        PositionEncoding::Utf8 => line_text.len() as u32,
    };
    
    Position { line, character }
}

/// Convert LSP Position to byte offset
pub fn position_to_offset(text: &str, position: Position, encoding: PositionEncoding) -> Option<usize> {
    let mut current_line = 0u32;
    let mut line_start = 0usize;
    
    for (i, c) in text.char_indices() {
        if current_line == position.line {
            // Found the line, now find character offset
            let line_end = text[i..].find('\n').map(|p| i + p).unwrap_or(text.len());
            let line_text = &text[line_start..line_end];
            
            let byte_offset = match encoding {
                PositionEncoding::Utf16 => {
                    utf16_offset_to_byte_offset(line_text, position.character as usize)
                }
                PositionEncoding::Utf8 => {
                    Some(position.character as usize)
                }
            };
            
            return byte_offset.map(|o| line_start + o.min(line_text.len()));
        }
        if c == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    
    // Position is at or past end
    if current_line == position.line {
        Some(text.len())
    } else {
        None
    }
}

fn utf16_offset_to_byte_offset(text: &str, utf16_offset: usize) -> Option<usize> {
    let mut utf16_count = 0usize;
    for (byte_idx, c) in text.char_indices() {
        if utf16_count >= utf16_offset {
            return Some(byte_idx);
        }
        utf16_count += c.len_utf16();
    }
    if utf16_count >= utf16_offset {
        Some(text.len())
    } else {
        None
    }
}
```

2. Negotiate encoding in `initialize()`:
```rust
// In initialize handler
let position_encoding = params
    .capabilities
    .general
    .as_ref()
    .and_then(|g| g.position_encodings.as_ref())
    .and_then(|encodings| {
        if encodings.contains(&PositionEncodingKind::UTF8) {
            Some(PositionEncoding::Utf8)
        } else {
            Some(PositionEncoding::Utf16)
        }
    })
    .unwrap_or(PositionEncoding::Utf16);

// Store in server state for use in all handlers
```

3. Update all position conversions to use the encoding module:
   - `v2_adapter.rs`: `span_to_range()`, `offset_to_position()`
   - `document.rs`: `offset_from_position()`, `position_from_offset()`
   - Remove duplicate implementations

---

### 1.3 Fix Comment Rendering Bug

**File**: `rust/crates/dsl-core/src/ast.rs`

**Problem**: `to_dsl_string()` renders comments with single semicolon but grammar 
requires double semicolon. Round-trip parse → render → parse will fail.

**Fix**:
```rust
// Line ~84-85, change:
Statement::Comment(c) => format!("; {}", c),

// To:
Statement::Comment(c) => format!(";; {}", c),

// Also fix to_user_dsl_string() at line ~93:
Statement::Comment(c) => format!(";; {}", c),
```

**Test**: Add round-trip test in parser_conformance.rs:
```rust
#[test]
fn test_comment_roundtrip() {
    let input = ";; This is a comment\n(cbu.create :name \"Fund\")";
    let program = parse_program(input).unwrap();
    let rendered = program.to_dsl_string();
    let reparsed = parse_program(&rendered).unwrap();
    assert_eq!(program, reparsed);
}
```

---

## Phase 2: AST Span Completeness

### 2.1 Add Spans to All AST Nodes

**File**: `rust/crates/dsl-core/src/ast.rs`

**Problem**: `Literal` values inside `AstNode::Literal` don't have spans. The adapter 
assigns parent's span to all children in lists/maps, causing "find expression at 
cursor" to return wrong nodes.

**Fix**: Refactor `AstNode` so every variant has a span:

```rust
/// AST Node - all possible node types in the tree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AstNode {
    /// Literal value with span
    Literal { value: Literal, span: Span },

    /// Symbol reference: @name
    SymbolRef { name: String, span: Span },

    /// Entity reference - needs gateway resolution
    EntityRef {
        entity_type: String,
        search_column: String,
        value: String,
        resolved_key: Option<String>,
        span: Span,
        ref_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        explain: Option<Box<ResolutionExplain>>,
    },

    /// List of nodes: [a, b, c]
    List { items: Vec<AstNode>, span: Span },

    /// Map of key-value pairs: {:key value}
    Map {
        entries: Vec<MapEntry>,  // See below
        span: Span,
    },

    /// Nested verb call
    Nested(Box<VerbCall>),
}

/// Map entry with span for the key
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapEntry {
    pub key: String,
    pub key_span: Span,
    pub value: AstNode,
}

impl AstNode {
    /// Get the span for any node
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

**Parser updates required** (`parser.rs`):
- `string_literal()` - capture span
- `number_literal()` - capture span
- `boolean_literal()` - capture span
- `null_literal()` - capture span
- `uuid_literal()` - capture span
- `symbol_ref()` - already has span tracking
- `list_literal()` - capture individual item spans
- `map_literal()` - capture key and value spans

---

### 2.2 Update v2_adapter to Use Node Spans

**File**: `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`

**Problem**: `convert_node()` receives a span parameter and uses it for all nodes,
ignoring actual node spans.

**Fix**: 
```rust
/// Convert v2 AstNode to LSP ParsedExpr.
fn convert_node(node: &AstNode, text: &str, encoding: PositionEncoding) -> ParsedExpr {
    // Use the node's own span, not a passed-in span
    let range = span_to_range(&node.span(), text, encoding);

    let kind = match node {
        AstNode::Literal { value, .. } => match value {
            Literal::String(s) => ExprKind::String { value: s.clone() },
            // ... rest of literal handling
        },

        AstNode::List { items, .. } => {
            // Convert each item using ITS OWN span
            let converted: Vec<ParsedExpr> = items
                .iter()
                .map(|item| convert_node(item, text, encoding))
                .collect();
            ExprKind::List { items: converted }
        }

        AstNode::Map { entries, .. } => {
            let converted: Vec<(String, ParsedExpr)> = entries
                .iter()
                .map(|entry| {
                    let value_expr = convert_node(&entry.value, text, encoding);
                    (entry.key.clone(), value_expr)
                })
                .collect();
            ExprKind::Map { entries: converted }
        }
        // ... rest
    };

    ParsedExpr { range, kind }
}
```

---

## Phase 3: Error Reporting

### 3.1 Capture Location in Parse Errors

**File**: `rust/crates/dsl-core/src/parser.rs`

**Problem**: Parse errors don't include location information. `extract_error_range()` 
always returns `Range::default()`.

**Fix**: With `nom_locate`, errors automatically include position. Create a custom 
error type:

```rust
use nom_locate::LocatedSpan;
use nom::error::{VerboseError, VerboseErrorKind};

pub type Span<'a> = LocatedSpan<&'a str>;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Option<(usize, usize)>,  // (line, column)
}

impl ParseError {
    pub fn from_nom_error(input: &str, err: VerboseError<Span>) -> Self {
        if let Some((span, kind)) = err.errors.first() {
            let line = span.location_line() as usize;
            let column = span.get_column();
            let message = format_error_kind(kind);
            ParseError {
                message,
                span: Some((line, column)),
            }
        } else {
            ParseError {
                message: "Parse error".to_string(),
                span: None,
            }
        }
    }
}

fn format_error_kind(kind: &VerboseErrorKind) -> String {
    match kind {
        VerboseErrorKind::Context(ctx) => format!("Expected {}", ctx),
        VerboseErrorKind::Char(c) => format!("Expected '{}'", c),
        VerboseErrorKind::Nom(ek) => format!("Parse error: {:?}", ek),
    }
}
```

**Update `parse_program()` return type**:
```rust
pub fn parse_program(input: &str) -> Result<Program, ParseError> {
    // ...
}
```

---

### 3.2 Update v2_adapter Error Handling

**File**: `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`

**Fix**: Use the new `ParseError` type:

```rust
pub fn parse_with_v2(text: &str, encoding: PositionEncoding) -> (DocumentState, Vec<Diagnostic>) {
    let mut state = DocumentState::new(text.to_string());
    let mut diagnostics = Vec::new();

    match parse_program(text) {
        Ok(program) => {
            state.expressions = convert_program(&program, text, encoding);
            extract_symbols(/* ... */);
        }
        Err(err) => {
            let range = if let Some((line, col)) = err.span {
                Range {
                    start: Position { 
                        line: (line - 1) as u32,  // nom_locate is 1-indexed
                        character: (col - 1) as u32,
                    },
                    end: Position {
                        line: (line - 1) as u32,
                        character: col as u32 + 10,  // Highlight ~10 chars
                    },
                }
            } else {
                Range::default()
            };
            
            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E001".to_string())),
                source: Some("dsl".to_string()),
                message: err.message,
                ..Default::default()
            });
        }
    }

    (state, diagnostics)
}
```

---

## Phase 4: Completion Context Fix

### 4.1 Fix Multiline Context Detection

**File**: `rust/crates/dsl-lsp/src/analysis/context.rs`

**Problem**: `detect_completion_context()` only scans the current line, breaking 
for multiline verb calls.

**Fix**:
```rust
/// Detect the completion context at a position.
pub fn detect_completion_context(doc: &DocumentState, position: Position) -> CompletionContext {
    // Convert position to byte offset and scan from document start
    let offset = match doc.offset_from_position(position) {
        Some(o) => o,
        None => {
            tracing::debug!("Could not convert position to offset");
            return CompletionContext::None;
        }
    };
    
    let prefix = &doc.text[..offset];
    
    tracing::debug!(
        "Context detection: position={:?}, offset={}, prefix_len={}",
        position, offset, prefix.len()
    );

    // Check for @ symbol - could be existing symbol ref OR entity lookup
    if let Some(at_pos) = prefix.rfind('@') {
        let after_at = &prefix[at_pos + 1..];
        if after_at
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            let before_at = &prefix[..at_pos];
            let (verb_name, keyword) = parse_sexp_context(before_at);
            // ... rest of @ handling
        }
    }

    // Find the enclosing s-expression from full prefix
    let (verb_name, current_keyword) = parse_sexp_context(prefix);
    
    // ... rest of context detection
}
```

**Remove line-based scanning**:
- Delete `doc.get_line(position.line)` usage
- Update all context detection to use full document prefix

---

## Phase 5: Tree-sitter Enhancements

### 5.1 Add Explicit `:as` Binding Rule

**File**: `rust/crates/dsl-lsp/tree-sitter-dsl/grammar.js`

**Problem**: Grammar doesn't distinguish `:as @symbol` binding from other keywords,
limiting syntax highlighting options.

**Fix**:
```javascript
module.exports = grammar({
  name: "dsl",

  extras: ($) => [/\s/, $.comment],

  rules: {
    source_file: ($) => repeat($._statement),

    _statement: ($) => choice($.verb_call, $.comment),

    // Renamed from 'list' for clarity
    verb_call: ($) => seq(
      "(",
      optional($.verb_name),
      repeat($._expression),
      optional($.as_binding),  // Explicit binding
      ")"
    ),

    // New: explicit :as binding rule
    as_binding: ($) => seq(
      ":as",
      $.symbol_ref
    ),

    _expression: ($) =>
      choice(
        $.keyword_arg,  // :keyword value pairs
        $.string,
        $.number,
        $.boolean,
        $.null_literal,
        $.symbol_ref,
        $.array,
        $.map,
        $.verb_call,  // nested calls
      ),

    // Keyword argument (not :as)
    keyword_arg: ($) => seq(
      $.keyword,
      $._value
    ),

    _value: ($) => choice(
      $.string,
      $.number,
      $.boolean,
      $.null_literal,
      $.symbol_ref,
      $.array,
      $.map,
      $.verb_call,
    ),

    verb_name: ($) => /[a-zA-Z_][a-zA-Z0-9_\-]*\.[a-zA-Z_][a-zA-Z0-9_\-]*/,

    keyword: ($) => seq(":", /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    string: ($) => seq('"', repeat(choice(/[^"\\]+/, /\\./)), '"'),

    number: ($) => /\-?[0-9]+(\.[0-9]+)?/,

    boolean: ($) => choice("true", "false"),

    null_literal: ($) => "nil",

    symbol_ref: ($) => seq("@", /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    array: ($) =>
      seq(
        "[",
        optional(seq($._value, repeat(seq(optional(","), $._value)))),
        "]",
      ),

    map: ($) => seq("{", repeat(seq($.keyword, $._value)), "}"),

    // Double semicolon comments
    comment: ($) => /;;[^\n]*/,
  },
});
```

**After changes**: Regenerate parser:
```bash
cd rust/crates/dsl-lsp/tree-sitter-dsl
npx tree-sitter generate
```

---

## Phase 6: Additional LSP Features

### 6.1 Implement `textDocument/documentSymbol`

**File**: `rust/crates/dsl-lsp/src/handlers/document_symbol.rs` (new file)

**Purpose**: Provide outline view in Zed sidebar.

```rust
use tower_lsp::lsp_types::*;
use crate::analysis::{DocumentState, ExprKind};

pub fn get_document_symbols(doc: &DocumentState) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    
    for expr in &doc.expressions {
        if let ExprKind::Call { verb_name, verb_range, args, .. } = &expr.kind {
            // Find :as binding if present
            let binding_name = args.iter()
                .find(|a| a.keyword == ":as")
                .and_then(|a| a.value.as_ref())
                .and_then(|v| match &v.kind {
                    ExprKind::SymbolRef { name } => Some(name.clone()),
                    _ => None,
                });
            
            let name = if let Some(ref sym) = binding_name {
                format!("@{} = {}", sym, verb_name)
            } else {
                verb_name.clone()
            };
            
            let kind = if binding_name.is_some() {
                SymbolKind::VARIABLE
            } else {
                SymbolKind::FUNCTION
            };
            
            #[allow(deprecated)]  // DocumentSymbol.deprecated field
            symbols.push(DocumentSymbol {
                name,
                detail: Some(verb_name.clone()),
                kind,
                tags: None,
                deprecated: None,
                range: expr.range,
                selection_range: *verb_range,
                children: None,
            });
        }
    }
    
    symbols
}
```

**Register handler** in main LSP server.

---

### 6.2 Implement `textDocument/hover`

**File**: `rust/crates/dsl-lsp/src/handlers/hover.rs` (new file)

```rust
use tower_lsp::lsp_types::*;
use crate::analysis::{DocumentState, ExprKind};
use ob_poc::dsl_v2::find_unified_verb;

pub fn get_hover(doc: &DocumentState, position: Position) -> Option<Hover> {
    let expr = doc.find_expr_at_position(position)?;
    
    match &expr.kind {
        ExprKind::Call { verb_name, .. } => {
            let parts: Vec<&str> = verb_name.split('.').collect();
            if parts.len() == 2 {
                if let Some(verb) = find_unified_verb(parts[0], parts[1]) {
                    let mut content = format!("**{}**\n\n{}", verb_name, verb.description);
                    
                    // Add required args
                    let required = verb.required_arg_names();
                    if !required.is_empty() {
                        content.push_str("\n\n**Required:**\n");
                        for arg in required {
                            content.push_str(&format!("- `:{}`\n", arg));
                        }
                    }
                    
                    // Add optional args
                    let optional = verb.optional_arg_names();
                    if !optional.is_empty() {
                        content.push_str("\n**Optional:**\n");
                        for arg in optional {
                            content.push_str(&format!("- `:{}`\n", arg));
                        }
                    }
                    
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: content,
                        }),
                        range: Some(expr.range),
                    });
                }
            }
        }
        ExprKind::SymbolRef { name } => {
            if let Some(def) = doc.get_symbol_def(name) {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "**@{}**\n\nType: `{}`\nDefined by: `{}`",
                            name, def.id_type, def.defined_by
                        ),
                    }),
                    range: Some(expr.range),
                });
            }
        }
        _ => {}
    }
    
    None
}
```

---

### 6.3 Thread Safety for Symbol Table

**File**: `rust/crates/dsl-lsp/src/analysis/mod.rs`

**Problem**: `SymbolTable` is not thread-safe for potential parallel document processing.

**Fix**:
```rust
use std::sync::Arc;
use dashmap::DashMap;

pub struct SymbolTable {
    symbols: DashMap<String, SymbolInfo>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self { symbols: DashMap::new() }
    }
    
    pub fn insert(&self, name: String, info: SymbolInfo) {
        self.symbols.insert(name, info);
    }
    
    pub fn get(&self, name: &str) -> Option<SymbolInfo> {
        self.symbols.get(name).map(|r| r.clone())
    }
    
    pub fn all(&self) -> impl Iterator<Item = (String, SymbolInfo)> + '_ {
        self.symbols.iter().map(|r| (r.key().clone(), r.value().clone()))
    }
}

// Add to Cargo.toml:
// dashmap = "5"
```

---

## Phase 7: Minor Fixes

### 7.1 Change Unknown Type Fallback

**File**: `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`

**Problem**: `infer_id_type()` returns `"uuid"` for unknown verbs, which is misleading.

**Fix**:
```rust
// Line ~356, change:
"uuid".to_string()

// To:
"unknown".to_string()
```

---

## Testing Checklist

After implementing all phases, verify:

### Position/Span Tests
- [ ] Multi-statement file: second statement span starts after first
- [ ] Nested verb call: inner span is absolute, not relative to outer
- [ ] List items: each item has distinct span
- [ ] Map entries: keys and values have distinct spans
- [ ] UTF-16 with emoji: `(cbu.create :name "Test 🎉 Fund")` - positions correct
- [ ] Multiline verb call: completion works on line 3 of 5-line call

### LSP Feature Tests
- [ ] Go-to-definition: `@fund` jumps to `:as @fund` location
- [ ] Find references: `@fund` finds all usages
- [ ] Hover on verb: shows description and args
- [ ] Hover on symbol: shows type and definition location
- [ ] Document symbols: outline shows all verb calls with bindings
- [ ] Completion after `@`: shows symbols with type-appropriate ranking
- [ ] Completion after `:`: shows verb-specific keywords

### Error Reporting Tests
- [ ] Unclosed paren: error at actual location, not (0,0)
- [ ] Invalid keyword: error highlights the keyword
- [ ] Missing value: error at end of keyword

### Round-trip Tests
- [ ] Parse → render → parse produces identical AST
- [ ] Comments preserved with `;;` prefix
- [ ] All literal types survive round-trip

---

## Dependency Order Summary

```
Phase 1.1 (nom_locate)
    ↓
Phase 1.2 (UTF-16 encoding)
    ↓
Phase 2.1 (AST node spans) ←── Phase 1.3 (comment bug - independent)
    ↓
Phase 2.2 (adapter update)
    ↓
Phase 3.1 (parse error location) 
    ↓
Phase 3.2 (adapter error handling)
    ↓
Phase 4.1 (multiline context)
    ↓
Phase 5.1 (tree-sitter :as rule) ←── independent, can be done anytime
    ↓
Phase 6.* (additional features) ←── independent, can be done anytime
    ↓
Phase 7.1 (minor fixes) ←── independent, can be done anytime
```

---

## Files Modified Summary

| File | Changes |
|------|---------|
| `dsl-core/Cargo.toml` | Add `nom_locate = "4"` |
| `dsl-core/src/parser.rs` | nom_locate integration, Span type, error location |
| `dsl-core/src/ast.rs` | Add spans to all AstNode variants, fix comment rendering |
| `dsl-lsp/Cargo.toml` | Add `dashmap = "5"` |
| `dsl-lsp/src/encoding.rs` | New: UTF-16/UTF-8 position encoding |
| `dsl-lsp/src/lib.rs` | Negotiate position encoding in initialize |
| `dsl-lsp/src/analysis/document.rs` | Use encoding module |
| `dsl-lsp/src/analysis/v2_adapter.rs` | Use node spans, encoding, better errors |
| `dsl-lsp/src/analysis/context.rs` | Scan full document prefix |
| `dsl-lsp/src/analysis/mod.rs` | Thread-safe SymbolTable |
| `dsl-lsp/src/handlers/document_symbol.rs` | New: outline support |
| `dsl-lsp/src/handlers/hover.rs` | New: hover info |
| `dsl-lsp/tree-sitter-dsl/grammar.js` | Explicit :as binding rule |
| `dsl-lsp/tests/parser_conformance.rs` | Add round-trip, span, encoding tests |

---

## Notes on Entity Resolution

Entity resolution (converting `"Apex Fund"` → UUID) is **not** an LSP concern.
The LSP validates:
- **Tier 1**: Syntax (NOM parser) - synchronous, always
- **Tier 2**: Schema (YAML verb registry) - synchronous, best-effort

Entity resolution is **Tier 3** (execution-time):
- Requires network round-trip to EntityGateway
- Database state dependent
- May require human disambiguation

The LSP can *assist* with entity completion (using `EntityLookupClient`) but
cannot *guarantee* resolution will succeed at execution time. This is the
correct separation for regulated finance - audit trail distinguishes:
- "DSL was syntactically valid at T1" (LSP/parser)
- "Entity resolution succeeded at T2" (execution)
- "These UUIDs were used" (audit)
