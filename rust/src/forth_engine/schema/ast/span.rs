//! Source span types for error reporting.

use serde::{Deserialize, Serialize};

/// Source location for error reporting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset from start of source
    pub start: usize,
    /// Byte offset of end (exclusive)
    pub end: usize,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
}

impl Span {
    /// Create a new span.
    pub fn new(start: usize, end: usize, line: u32, column: u32) -> Self {
        Self { start, end, line, column }
    }

    /// Create a span from byte offsets only.
    pub fn from_offsets(start: usize, end: usize) -> Self {
        Self { start, end, line: 0, column: 0 }
    }

    /// Merge two spans into one covering both.
    pub fn merge(a: &Span, b: &Span) -> Span {
        Span {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
            line: a.line.min(b.line),
            column: if a.line <= b.line { a.column } else { b.column },
        }
    }

    /// Check if this span contains a byte offset.
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }

    /// Get the length of this span in bytes.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if span is empty.
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// A value with its source span attached.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

/// Convert source offset to line and column.
pub fn offset_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    
    (line, col)
}

/// Get the source text of a specific line.
pub fn get_source_line(source: &str, line: u32) -> &str {
    source.lines()
        .nth((line.saturating_sub(1)) as usize)
        .unwrap_or("")
}

/// Convert span to line/column using source text.
pub fn span_to_line_col(source: &str, span: &Span) -> (u32, u32) {
    if span.line > 0 {
        (span.line, span.column)
    } else {
        offset_to_line_col(source, span.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_merge() {
        let a = Span::new(0, 5, 1, 1);
        let b = Span::new(10, 15, 1, 11);
        let merged = Span::merge(&a, &b);
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 15);
    }

    #[test]
    fn test_offset_to_line_col() {
        let source = "line1\nline2\nline3";
        assert_eq!(offset_to_line_col(source, 0), (1, 1));
        assert_eq!(offset_to_line_col(source, 6), (2, 1));
        assert_eq!(offset_to_line_col(source, 12), (3, 1));
    }

    #[test]
    fn test_get_source_line() {
        let source = "line1\nline2\nline3";
        assert_eq!(get_source_line(source, 1), "line1");
        assert_eq!(get_source_line(source, 2), "line2");
        assert_eq!(get_source_line(source, 3), "line3");
    }
}
