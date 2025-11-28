//! Document state tracking.

#![allow(dead_code)]

use tower_lsp::lsp_types::{Position, Range};

/// Parsed expression in the document.
#[derive(Debug, Clone)]
pub struct ParsedExpr {
    /// Expression type
    pub kind: ExprKind,
    /// Source range
    pub range: Range,
}

/// Kind of expression.
#[derive(Debug, Clone)]
pub enum ExprKind {
    /// S-expression: (verb-name :arg value ...)
    Call {
        verb_name: String,
        verb_range: Range,
        args: Vec<ParsedArg>,
    },
    /// Symbol reference: @name
    SymbolRef { name: String },
    /// String literal
    String { value: String },
    /// Number literal
    Number { value: String },
    /// Identifier (true, false, nil, etc.)
    Identifier { value: String },
    /// List: [...]
    List { items: Vec<ParsedExpr> },
    /// Comment
    Comment { text: String },
}

/// Parsed argument in a call.
#[derive(Debug, Clone)]
pub struct ParsedArg {
    /// Keyword name (e.g., ":cbu-name")
    pub keyword: String,
    /// Keyword range
    pub keyword_range: Range,
    /// Value expression
    pub value: Option<Box<ParsedExpr>>,
}

/// State of a parsed document.
#[derive(Debug, Clone)]
pub struct DocumentState {
    /// Full document text
    pub text: String,
    /// Parsed expressions
    pub expressions: Vec<ParsedExpr>,
    /// Symbol definitions in this document
    pub symbol_defs: Vec<SymbolDef>,
    /// Symbol references in this document
    pub symbol_refs: Vec<SymbolRef>,
}

/// Symbol definition location.
#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub name: String,
    pub range: Range,
    pub defined_by: String,
    pub id_type: String,
}

/// Symbol reference location.
#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub name: String,
    pub range: Range,
}

impl DocumentState {
    /// Create a new empty document state.
    pub fn new(text: String) -> Self {
        Self {
            text,
            expressions: Vec::new(),
            symbol_defs: Vec::new(),
            symbol_refs: Vec::new(),
        }
    }

    /// Get the line at a position.
    pub fn get_line(&self, line: u32) -> Option<&str> {
        self.text.lines().nth(line as usize)
    }

    /// Get text at a range.
    pub fn get_text_at_range(&self, range: Range) -> Option<String> {
        let start = self.offset_from_position(range.start)?;
        let end = self.offset_from_position(range.end)?;
        self.text.get(start..end).map(|s| s.to_string())
    }

    /// Convert position to byte offset.
    pub fn offset_from_position(&self, position: Position) -> Option<usize> {
        let mut offset = 0;
        for (line_num, line) in self.text.lines().enumerate() {
            if line_num == position.line as usize {
                let char_offset: usize = line
                    .chars()
                    .take(position.character as usize)
                    .map(|c| c.len_utf8())
                    .sum();
                return Some(offset + char_offset);
            }
            offset += line.len() + 1; // +1 for newline
        }
        None
    }

    /// Convert byte offset to position.
    pub fn position_from_offset(&self, offset: usize) -> Position {
        let mut current_offset = 0;
        for (line_num, line) in self.text.lines().enumerate() {
            let line_end = current_offset + line.len();
            if offset <= line_end {
                let char_offset = line[..(offset - current_offset).min(line.len())]
                    .chars()
                    .count();
                return Position {
                    line: line_num as u32,
                    character: char_offset as u32,
                };
            }
            current_offset = line_end + 1; // +1 for newline
        }
        Position {
            line: self.text.lines().count() as u32,
            character: 0,
        }
    }

    /// Find the expression at a position.
    pub fn find_expr_at_position(&self, position: Position) -> Option<&ParsedExpr> {
        self.expressions
            .iter()
            .find(|e| contains_position(&e.range, position))
    }

    /// Find verb call at position.
    pub fn find_call_at_position(&self, position: Position) -> Option<(&str, &[ParsedArg])> {
        for expr in &self.expressions {
            if let ExprKind::Call {
                verb_name, args, ..
            } = &expr.kind
            {
                if contains_position(&expr.range, position) {
                    return Some((verb_name.as_str(), args.as_slice()));
                }
            }
        }
        None
    }

    /// Get symbol definition by name.
    pub fn get_symbol_def(&self, name: &str) -> Option<&SymbolDef> {
        self.symbol_defs.iter().find(|s| s.name == name)
    }
}

/// Check if a range contains a position.
pub fn contains_position(range: &Range, position: Position) -> bool {
    if position.line < range.start.line || position.line > range.end.line {
        return false;
    }
    if position.line == range.start.line && position.character < range.start.character {
        return false;
    }
    if position.line == range.end.line && position.character > range.end.character {
        return false;
    }
    true
}
