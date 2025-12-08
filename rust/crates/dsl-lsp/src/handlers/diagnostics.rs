//! Diagnostics handler for the DSL Language Server.
//!
//! Uses the unified LspValidator from ob_poc for semantic validation.
//! This ensures LSP and Server use the SAME validation pipeline.

use tower_lsp::lsp_types::*;

use crate::analysis::document::DocumentState;
use crate::analysis::parse_with_v2;

use ob_poc::dsl_v2::validation::{Severity, ValidationContext};
use ob_poc::dsl_v2::LspValidator;

/// Analyze a document with full semantic validation via EntityGateway.
///
/// This is the primary validation path - uses the same validator as the server.
pub async fn analyze_document_async(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    // Step 1: Parse to get DocumentState (for LSP features like symbols, completions)
    let (state, mut diagnostics) = parse_with_v2(text);

    // Step 2: Run full semantic validation via EntityGateway
    match LspValidator::connect().await {
        Ok(mut validator) => {
            let context = ValidationContext::default();
            let (semantic_diags, _validated) = validator.validate(text, &context).await;

            // Convert semantic diagnostics to LSP format
            for diag in semantic_diags {
                diagnostics.push(convert_diagnostic(&diag, text));
            }
        }
        Err(e) => {
            // EntityGateway not available - fall back to syntax-only validation
            tracing::warn!(
                "EntityGateway not available for validation: {}. Using syntax-only mode.",
                e
            );
            // The parse_with_v2 diagnostics are still useful
        }
    }

    (state, diagnostics)
}

/// Synchronous fallback for when async isn't available.
/// Only performs syntax validation (no EntityGateway).
pub fn analyze_document(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    // Use v2 parser via adapter - syntax validation only
    parse_with_v2(text)
}

/// Convert internal Diagnostic to LSP Diagnostic format
fn convert_diagnostic(diag: &ob_poc::dsl_v2::validation::Diagnostic, source: &str) -> Diagnostic {
    // Convert SourceSpan to LSP Range
    let range = span_to_range(&diag.span, source);

    // Convert severity
    let severity = match diag.severity {
        Severity::Error => Some(DiagnosticSeverity::ERROR),
        Severity::Warning => Some(DiagnosticSeverity::WARNING),
        Severity::Hint => Some(DiagnosticSeverity::HINT),
    };

    // Build message with suggestions if any
    let message = if diag.suggestions.is_empty() {
        diag.message.clone()
    } else {
        let suggestions: Vec<String> = diag
            .suggestions
            .iter()
            .map(|s| format!("'{}' ({:.0}%)", s.replacement, s.confidence * 100.0))
            .collect();
        format!(
            "{}. Did you mean: {}?",
            diag.message,
            suggestions.join(", ")
        )
    };

    Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(diag.code.as_str().to_string())),
        source: Some("dsl-lsp".to_string()),
        message,
        related_information: None,
        tags: None,
        code_description: None,
        data: None,
    }
}

/// Convert SourceSpan to LSP Range
fn span_to_range(span: &ob_poc::dsl_v2::validation::SourceSpan, source: &str) -> Range {
    // Calculate end position
    let start_line = span.line.saturating_sub(1); // LSP is 0-indexed
    let start_char = span.column;

    // Find end line and column
    let mut end_line = start_line;
    let mut end_char = start_char + span.length;

    // Check if span crosses lines
    let start_offset = span.offset as usize;
    let end_offset = start_offset + span.length as usize;

    if end_offset <= source.len() {
        let span_text = &source[start_offset..end_offset];
        for ch in span_text.chars() {
            if ch == '\n' {
                end_line += 1;
                end_char = 0;
            } else {
                end_char += 1;
            }
        }
        // Reset to start of span text for correct end_char
        if span_text.contains('\n') {
            end_char = span_text
                .lines()
                .last()
                .map(|l| l.len() as u32)
                .unwrap_or(0);
        }
    }

    Range {
        start: Position {
            line: start_line,
            character: start_char,
        },
        end: Position {
            line: end_line,
            character: end_char,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_to_range_single_line() {
        let source = "(cbu.ensure :name \"Test\")";
        let span = ob_poc::dsl_v2::validation::SourceSpan {
            line: 1,
            column: 1,
            offset: 1,
            length: 10,
        };
        let range = span_to_range(&span, source);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 1);
    }
}
