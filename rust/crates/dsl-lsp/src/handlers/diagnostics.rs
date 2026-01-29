//! Diagnostics handler for the DSL Language Server.
//!
//! Uses the unified LspValidator from ob_poc for semantic validation,
//! plus the planning facade for DAG-based analysis.
//!
//! This ensures LSP and Server use the SAME validation pipeline.

#![allow(dead_code)] // Public API - functions may be used by LSP server

use std::sync::Arc;
use tower_lsp::lsp_types::*;

use crate::analysis::document::DocumentState;
use crate::analysis::parse_with_v2;
use crate::encoding::{span_to_range as encoding_span_to_range, PositionEncoding};

use ob_poc::dsl_v2::config::ConfigLoader;
use ob_poc::dsl_v2::planning_facade::{analyse_and_plan, PlanningInput};
use ob_poc::dsl_v2::runtime_registry::RuntimeVerbRegistry;
use ob_poc::dsl_v2::validation::{Severity, ValidationContext};
use ob_poc::dsl_v2::LspValidator;

/// Create a planning registry from config
/// This is cached after first call via lazy_static pattern
/// Returns None if config can't be loaded (e.g., LSP launched without proper working dir)
fn create_planning_registry() -> Option<Arc<RuntimeVerbRegistry>> {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<Option<Arc<RuntimeVerbRegistry>>> = OnceLock::new();

    REGISTRY
        .get_or_init(|| {
            let loader = ConfigLoader::from_env();
            match loader.load_verbs() {
                Ok(config) => {
                    let registry = RuntimeVerbRegistry::from_config(&config);
                    tracing::info!("Loaded {} verbs for planning", registry.all_verbs().count());
                    Some(Arc::new(registry))
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not load verb config for planning diagnostics: {}. \
                         Planning diagnostics will be disabled.",
                        e
                    );
                    None
                }
            }
        })
        .clone()
}

/// Result of document analysis including planning info for code actions
pub struct AnalysisResult {
    pub state: DocumentState,
    pub diagnostics: Vec<Diagnostic>,
    pub planning_output: ob_poc::dsl_v2::planning_facade::PlanningOutput,
    /// Semantic diagnostics with entity suggestions (for code actions)
    pub semantic_diagnostics: Vec<ob_poc::dsl_v2::validation::Diagnostic>,
}

/// Analyze a document with full semantic validation via EntityGateway.
///
/// This is the primary validation path - uses the same validator as the server,
/// plus the planning facade for DAG-based dependency analysis.
pub async fn analyze_document_async(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    let result = analyze_document_full(text).await;
    (result.state, result.diagnostics)
}

/// Full analysis returning planning output for code actions
pub async fn analyze_document_full(text: &str) -> AnalysisResult {
    // Step 1: Parse to get DocumentState (for LSP features like symbols, completions)
    let (state, mut diagnostics) = parse_with_v2(text);

    // Step 2: Run full semantic validation via EntityGateway
    let mut semantic_diagnostics = Vec::new();
    match LspValidator::connect().await {
        Ok(mut validator) => {
            let context = ValidationContext::default();
            let (semantic_diags, _validated) = validator.validate(text, &context).await;

            // Store semantic diagnostics for code actions (entity suggestions)
            semantic_diagnostics = semantic_diags.clone();

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

    // Step 3: Run planning facade for DAG-based analysis
    // This catches reordering issues, cycle detection, and provides plan info
    // ONLY run if:
    // - There are no parse errors (incomplete code generates false positives)
    // - Verb config was loaded successfully
    let has_parse_errors = diagnostics
        .iter()
        .any(|d| d.severity == Some(DiagnosticSeverity::ERROR));

    let planning_output = if !has_parse_errors {
        if let Some(registry) = create_planning_registry() {
            let planning_input = PlanningInput::new(text, registry);
            let output = analyse_and_plan(planning_input);

            // Convert planning diagnostics to LSP format
            for diag in &output.diagnostics {
                diagnostics.push(convert_planning_diagnostic(diag, text));
            }
            output
        } else {
            // No registry available - skip planning diagnostics
            ob_poc::dsl_v2::planning_facade::PlanningOutput::default()
        }
    } else {
        // Return empty planning output for incomplete code
        ob_poc::dsl_v2::planning_facade::PlanningOutput::default()
    };

    AnalysisResult {
        state,
        diagnostics,
        planning_output,
        semantic_diagnostics,
    }
}

/// Synchronous fallback for when async isn't available.
/// Only performs syntax validation (no EntityGateway).
pub fn analyze_document(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    // Use v2 parser via adapter - syntax validation only
    parse_with_v2(text)
}

/// Convert internal Diagnostic (from validation module) to LSP Diagnostic format
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

/// Convert planning facade Diagnostic to LSP Diagnostic format
fn convert_planning_diagnostic(
    diag: &ob_poc::dsl_v2::diagnostics::Diagnostic,
    _source: &str,
) -> Diagnostic {
    use ob_poc::dsl_v2::diagnostics::Severity as PlanningSeverity;

    // Convert span to LSP Range
    let range = if let Some(ref span) = diag.span {
        Range {
            start: Position {
                line: span.start_line.saturating_sub(1), // LSP is 0-indexed
                character: span.start_col.saturating_sub(1),
            },
            end: Position {
                line: span.end_line.saturating_sub(1),
                character: span.end_col.saturating_sub(1),
            },
        }
    } else {
        // No span - use start of document
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        }
    };

    // Convert severity
    let severity = match diag.severity {
        PlanningSeverity::Error => Some(DiagnosticSeverity::ERROR),
        PlanningSeverity::Warning => Some(DiagnosticSeverity::WARNING),
        PlanningSeverity::Hint => Some(DiagnosticSeverity::HINT),
        PlanningSeverity::Info => Some(DiagnosticSeverity::INFORMATION),
    };

    // Generate diagnostic code string
    let code_str = format!("{:?}", diag.code);

    Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(code_str)),
        source: Some("dsl-planner".to_string()),
        message: diag.message.clone(),
        related_information: None,
        tags: None,
        code_description: None,
        data: None,
    }
}

/// Convert SourceSpan to LSP Range using proper UTF-16 encoding
fn span_to_range(span: &ob_poc::dsl_v2::validation::SourceSpan, source: &str) -> Range {
    // Use byte offsets directly with the encoding module for proper UTF-16 handling
    let start_offset = span.offset as usize;
    let end_offset = start_offset + span.length as usize;

    // Use the encoding module for proper UTF-16 position calculation
    encoding_span_to_range(start_offset, end_offset, source, PositionEncoding::Utf16)
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
