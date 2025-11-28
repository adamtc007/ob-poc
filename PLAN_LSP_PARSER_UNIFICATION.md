# Plan: LSP Parser Unification with DSL v2

**Created:** 2025-11-28  
**Status:** PLANNED  
**Priority:** P1 — Technical Debt / Consistency  
**Goal:** Replace LSP's hand-rolled parser with dsl_v2 parser via adapter layer

---

## Problem Statement

The LSP currently has **two parsers**:
1. `dsl_v2/parser.rs` - Nom-based, source of truth for runtime
2. `dsl-lsp/src/handlers/diagnostics.rs` - Hand-rolled, LSP-specific

This creates:
- Risk of parsing divergence (LSP accepts what v2 rejects)
- Feature lag (v2's `NestedCall`, `:as` binding need separate LSP updates)
- Double maintenance burden

## Solution

Create an adapter layer that converts `dsl_v2::ast` types to LSP's `DocumentState`.

---

## Phase 1: Enhance v2 AST with Spans

The v2 AST needs more granular position tracking for LSP features (hover, go-to-definition).

### File: `rust/src/dsl_v2/ast.rs`

Add spans to `Argument`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub key: Key,
    pub key_span: Span,      // NEW: position of the keyword (e.g., `:name`)
    pub value: Value,
    pub value_span: Span,    // NEW: position of the value
}
```

Add domain/verb span to `VerbCall`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct VerbCall {
    pub domain: String,
    pub verb: String,
    pub verb_span: Span,     // NEW: position of "domain.verb" text
    pub arguments: Vec<Argument>,
    pub as_binding: Option<String>,
    pub as_binding_span: Option<Span>,  // NEW: position of `:as @symbol`
    pub span: Span,          // Full expression span (existing)
}
```

### File: `rust/src/dsl_v2/parser.rs`

Update parsers to capture spans:

```rust
// In argument parser
fn argument(input: Span) -> IResult<Span, Argument> {
    let (input, _) = multispace0(input)?;
    let key_start = input.location_offset();
    let (input, key) = key_parser(input)?;
    let key_end = input.location_offset();
    
    let (input, _) = multispace0(input)?;
    let value_start = input.location_offset();
    let (input, value) = value_parser(input)?;
    let value_end = input.location_offset();
    
    Ok((input, Argument {
        key,
        key_span: Span { start: key_start, end: key_end },
        value,
        value_span: Span { start: value_start, end: value_end },
    }))
}
```

---

## Phase 2: Create V2 Adapter Module

### New File: `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`

```rust
//! Adapter to convert dsl_v2 AST to LSP DocumentState.

use ob_poc::dsl_v2::{
    parse_program, Program, Statement, VerbCall, Value, Argument, Key, Span as V2Span,
};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};

use super::document::{DocumentState, ExprKind, ParsedArg, ParsedExpr, SymbolDef, SymbolRef};

/// Parse document using v2 parser and convert to LSP types.
pub fn parse_with_v2(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    let mut state = DocumentState::new(text.to_string());
    let mut diagnostics = Vec::new();

    match parse_program(text) {
        Ok(program) => {
            state.expressions = convert_program(&program, text);
            extract_symbols(&state.expressions, &mut state.symbol_defs, &mut state.symbol_refs);
        }
        Err(err) => {
            // V2 parse error - convert to diagnostic
            diagnostics.push(Diagnostic {
                range: Range::default(), // TODO: extract span from error if available
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E000".to_string())),
                source: Some("dsl-lsp".to_string()),
                message: err.to_string(),
                ..Default::default()
            });
        }
    }

    (state, diagnostics)
}

/// Convert v2 Program to LSP expressions.
fn convert_program(program: &Program, text: &str) -> Vec<ParsedExpr> {
    program
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::VerbCall(vc) => Some(convert_verb_call(vc, text)),
            Statement::Comment(_) => None,
        })
        .collect()
}

/// Convert v2 VerbCall to LSP ParsedExpr.
fn convert_verb_call(vc: &VerbCall, text: &str) -> ParsedExpr {
    let verb_name = format!("{}.{}", vc.domain, vc.verb);
    let range = span_to_range(&vc.span, text);
    let verb_range = span_to_range(&vc.verb_span, text); // Uses new verb_span

    let args: Vec<ParsedArg> = vc
        .arguments
        .iter()
        .map(|arg| convert_argument(arg, text))
        .collect();

    ParsedExpr {
        range,
        kind: ExprKind::Call {
            verb_name,
            verb_range,
            args,
        },
    }
}

/// Convert v2 Argument to LSP ParsedArg.
fn convert_argument(arg: &Argument, text: &str) -> ParsedArg {
    let keyword = match &arg.key {
        Key::Simple(k) => format!(":{}", k),
        Key::Nested(parts) => format!(":{}", parts.join(".")),
    };
    let keyword_range = span_to_range(&arg.key_span, text);
    let value = Some(Box::new(convert_value(&arg.value, &arg.value_span, text)));

    ParsedArg {
        keyword,
        keyword_range,
        value,
    }
}

/// Convert v2 Value to LSP ParsedExpr.
fn convert_value(value: &Value, span: &V2Span, text: &str) -> ParsedExpr {
    let range = span_to_range(span, text);

    let kind = match value {
        Value::String(s) => ExprKind::String { value: s.clone() },
        Value::Number(n) => ExprKind::Number { value: n.to_string() },
        Value::Boolean(b) => ExprKind::Identifier { value: b.to_string() },
        Value::Symbol(s) => ExprKind::SymbolRef { name: s.clone() },
        Value::Null => ExprKind::Identifier { value: "nil".to_string() },
        Value::List(items) => {
            // TODO: need spans for each list item
            let converted: Vec<ParsedExpr> = items
                .iter()
                .map(|v| convert_value(v, span, text)) // Approximate span
                .collect();
            ExprKind::List { items: converted }
        }
        Value::NestedCall(vc) => {
            // Recursively convert nested verb call
            return convert_verb_call(vc, text);
        }
    };

    ParsedExpr { range, kind }
}

/// Convert v2 byte-offset Span to LSP line/column Range.
fn span_to_range(span: &V2Span, text: &str) -> Range {
    Range {
        start: offset_to_position(span.start, text),
        end: offset_to_position(span.end, text),
    }
}

/// Convert byte offset to LSP Position (line, character).
fn offset_to_position(offset: usize, text: &str) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;

    for (i, ch) in text.chars().enumerate() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    Position { line, character: col }
}

/// Extract symbol definitions and references from converted expressions.
fn extract_symbols(
    expressions: &[ParsedExpr],
    defs: &mut Vec<SymbolDef>,
    refs: &mut Vec<SymbolRef>,
) {
    for expr in expressions {
        extract_symbols_from_expr(expr, defs, refs);
    }
}

fn extract_symbols_from_expr(
    expr: &ParsedExpr,
    defs: &mut Vec<SymbolDef>,
    refs: &mut Vec<SymbolRef>,
) {
    match &expr.kind {
        ExprKind::Call { verb_name, args, .. } => {
            // Check for :as @symbol binding
            for arg in args {
                if arg.keyword == ":as" {
                    if let Some(value) = &arg.value {
                        if let ExprKind::SymbolRef { name } = &value.kind {
                            defs.push(SymbolDef {
                                name: name.clone(),
                                range: value.range,
                                defined_by: verb_name.clone(),
                                id_type: "uuid".to_string(),
                            });
                        }
                    }
                } else if let Some(value) = &arg.value {
                    extract_symbols_from_expr(value, defs, refs);
                }
            }
        }
        ExprKind::SymbolRef { name } => {
            refs.push(SymbolRef {
                name: name.clone(),
                range: expr.range,
            });
        }
        ExprKind::List { items } => {
            for item in items {
                extract_symbols_from_expr(item, defs, refs);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_parse() {
        let input = r#"(cbu.create :name "Test Fund")"#;
        let (state, diags) = parse_with_v2(input);
        
        assert!(diags.is_empty(), "Expected no errors: {:?}", diags);
        assert_eq!(state.expressions.len(), 1);
        
        if let ExprKind::Call { verb_name, args, .. } = &state.expressions[0].kind {
            assert_eq!(verb_name, "cbu.create");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].keyword, ":name");
        } else {
            panic!("Expected Call expression");
        }
    }

    #[test]
    fn test_symbol_binding() {
        let input = r#"(cbu.create :name "Test" :as @mycbu)"#;
        let (state, diags) = parse_with_v2(input);
        
        assert!(diags.is_empty());
        assert_eq!(state.symbol_defs.len(), 1);
        assert_eq!(state.symbol_defs[0].name, "mycbu");
    }

    #[test]
    fn test_nested_calls() {
        let input = r#"(cbu.create :name "Fund" :roles [(cbu.assign-role :entity-id @e :role "Mgr")])"#;
        let (state, diags) = parse_with_v2(input);
        
        assert!(diags.is_empty(), "Errors: {:?}", diags);
        assert_eq!(state.expressions.len(), 1);
    }
}
```

---

## Phase 3: Update Diagnostics Handler

### File: `rust/crates/dsl-lsp/src/handlers/diagnostics.rs`

Replace `parse_document` calls with v2 adapter:

```rust
//! Diagnostics handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::document::{DocumentState, ExprKind, SymbolDef};
use crate::analysis::v2_adapter::parse_with_v2;  // NEW

use ob_poc::dsl_v2::{find_verb, STANDARD_VERBS};

/// Analyze a document and return state + diagnostics.
pub fn analyze_document(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    // Use v2 parser via adapter
    let (mut state, mut diagnostics) = parse_with_v2(text);

    // Validate expressions against verb schema
    for expr in &state.expressions {
        validate_expression(expr, &state.symbol_defs, &mut diagnostics);
    }

    // Check for undefined symbol references
    for sym_ref in &state.symbol_refs {
        if !state.symbol_defs.iter().any(|d| d.name == sym_ref.name) {
            diagnostics.push(Diagnostic {
                range: sym_ref.range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E007".to_string())),
                source: Some("dsl-lsp".to_string()),
                message: format!("undefined symbol '@{}'", sym_ref.name),
                ..Default::default()
            });
        }
    }

    (state, diagnostics)
}

// Keep validate_expression, validate_nested_expr - these use find_verb from v2
// DELETE: parse_document, parse_expression, parse_value, offset_to_position, skip_whitespace
```

---

## Phase 4: Update Module Exports

### File: `rust/crates/dsl-lsp/src/analysis/mod.rs`

```rust
mod context;
mod document;
mod symbols;
mod v2_adapter;  // NEW

pub use context::{detect_completion_context, CompletionContext};
pub use document::{contains_position, DocumentState, ExprKind, ParsedArg, ParsedExpr, SymbolDef, SymbolRef};
pub use symbols::SymbolTable;
pub use v2_adapter::parse_with_v2;  // NEW
```

---

## Phase 5: Add Parser Conformance Tests

### New File: `rust/crates/dsl-lsp/tests/parser_conformance.rs`

```rust
//! Tests ensuring LSP and v2 parsers produce equivalent results.

use ob_poc::dsl_v2::parse_program;
use dsl_lsp::analysis::parse_with_v2;

const VALID_INPUTS: &[&str] = &[
    r#"(cbu.create :name "Fund")"#,
    r#"(cbu.create :name "Fund" :jurisdiction "LU")"#,
    r#"(cbu.create :name "Fund" :as @cbu)"#,
    r#"(entity.create :name "Person" :type "NATURAL")"#,
    r#"(cbu.create :name "Fund" :roles [(cbu.assign-role :entity-id @e :role "Mgr")])"#,
    r#"
    (cbu.create :name "Fund" :as @fund)
    (entity.create :name "Co" :as @co)
    (cbu.attach-entity :cbu-id @fund :entity-id @co :role "Manager")
    "#,
];

const INVALID_INPUTS: &[&str] = &[
    r#"(cbu.create :name"#,           // Unclosed
    r#"cbu.create :name "Fund")"#,    // Missing open paren
    r#"(cbu.create name "Fund")"#,    // Missing colon on keyword
];

#[test]
fn valid_inputs_parse_successfully() {
    for input in VALID_INPUTS {
        let (_, diags) = parse_with_v2(input);
        assert!(diags.is_empty(), "Input should be valid:\n{}\nErrors: {:?}", input, diags);
        
        let v2_result = parse_program(input);
        assert!(v2_result.is_ok(), "V2 should also accept:\n{}", input);
    }
}

#[test]
fn invalid_inputs_produce_errors() {
    for input in INVALID_INPUTS {
        let (_, diags) = parse_with_v2(input);
        assert!(!diags.is_empty(), "Input should be invalid:\n{}", input);
    }
}
```

---

## Phase 6: Cleanup

After validation:

1. **Delete from `diagnostics.rs`:**
   - `parse_document()`
   - `parse_expression()`
   - `parse_value()`
   - `skip_whitespace()`
   - `offset_to_position()` (moved to adapter)
   - `ParseError` struct

2. **Run tests:**
   ```bash
   cargo test -p dsl-lsp
   cargo test -p ob-poc dsl_v2
   ```

3. **Build LSP:**
   ```bash
   cargo build -p dsl-lsp --release
   ```

---

## File Change Summary

| File | Action | Lines Changed |
|------|--------|---------------|
| `dsl_v2/ast.rs` | Modify | Add `key_span`, `value_span`, `verb_span` |
| `dsl_v2/parser.rs` | Modify | Capture spans in parsers |
| `dsl-lsp/src/analysis/v2_adapter.rs` | **Create** | ~200 lines |
| `dsl-lsp/src/analysis/mod.rs` | Modify | Add export |
| `dsl-lsp/src/handlers/diagnostics.rs` | Modify | Remove ~300 lines, add 1 import |
| `dsl-lsp/tests/parser_conformance.rs` | **Create** | ~50 lines |

---

## Verification Checklist

- [ ] v2 AST has spans for keywords, values, verb names
- [ ] v2 parser captures all spans
- [ ] Adapter converts v2 AST to LSP types
- [ ] All existing LSP tests pass
- [ ] Parser conformance tests pass
- [ ] LSP builds and runs
- [ ] Hover works on verb names
- [ ] Hover works on keywords
- [ ] Completions work
- [ ] Diagnostics show at correct positions
- [ ] Go-to-definition works for symbols

---

## Rollback Plan

If issues arise:
1. Revert `diagnostics.rs` to use old `parse_document()`
2. Keep `v2_adapter.rs` for future use
3. File issue documenting specific failures
