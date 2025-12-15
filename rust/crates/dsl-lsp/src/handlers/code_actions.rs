//! Code actions handler for the DSL Language Server.
//!
//! Provides quick fixes and refactoring suggestions:
//! - Implicit create actions for undefined symbols
//! - Reorder statements for dependency resolution
//! - Entity suggestion quick fixes for unresolved refs

use tower_lsp::lsp_types::*;

use ob_poc::dsl_v2::diagnostics::{DiagnosticCode, SuggestedFix};
use ob_poc::dsl_v2::planning_facade::{PlanningOutput, SyntheticStep};
use ob_poc::dsl_v2::validation::{Diagnostic as SemanticDiagnostic, Suggestion};

/// Generate code actions from planning output and semantic diagnostics
///
/// This produces:
/// - Quick fixes for implicit creates (undefined symbols)
/// - Refactoring actions for reordering
/// - Entity suggestion quick fixes (e.g., "Did you mean 'John Smith'?")
pub fn get_code_actions(
    planning_output: &PlanningOutput,
    semantic_diagnostics: &[SemanticDiagnostic],
    range: Range,
    uri: &Url,
    source: &str,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    // Add implicit create actions
    for step in &planning_output.synthetic_steps {
        if let Some(action) = create_implicit_create_action(step, uri, source) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    // Add reorder action if statements were reordered
    if planning_output.was_reordered {
        if let Some(action) = create_reorder_action(planning_output, uri, source, range) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    // Add actions from diagnostic suggested fixes
    for diag in &planning_output.diagnostics {
        if let Some(ref fix) = diag.suggested_fix {
            if let Some(action) = create_fix_action(diag, fix, uri, source) {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }
    }

    // Add quick fixes from semantic diagnostics (entity suggestions)
    for diag in semantic_diagnostics {
        for suggestion in &diag.suggestions {
            if let Some(action) = create_suggestion_action(diag, suggestion, uri, source) {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }
    }

    actions
}

/// Create a code action for an implicit create suggestion
fn create_implicit_create_action(
    step: &SyntheticStep,
    uri: &Url,
    _source: &str,
) -> Option<CodeAction> {
    // Calculate position to insert (before the statement that needs it)
    let insert_line = step.insert_before_stmt.saturating_sub(1) as u32;

    let insert_text = format!("{}\n", step.suggested_dsl);

    let edit = TextEdit {
        range: Range {
            start: Position {
                line: insert_line,
                character: 0,
            },
            end: Position {
                line: insert_line,
                character: 0,
            },
        },
        new_text: insert_text,
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    Some(CodeAction {
        title: format!(
            "Create {} '{}' with {}",
            step.entity_type, step.binding, step.canonical_verb
        ),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(true),
        disabled: None,
        data: None,
    })
}

/// Create a code action to reorder statements
fn create_reorder_action(
    planning_output: &PlanningOutput,
    uri: &Url,
    source: &str,
    _range: Range,
) -> Option<CodeAction> {
    // Get the planned execution order
    let plan = planning_output.plan.as_ref()?;

    // Build reordered source from ops
    let mut reordered_lines: Vec<String> = Vec::new();
    let source_lines: Vec<&str> = source.lines().collect();

    // Track which source statements we've already added
    let mut seen_stmts = std::collections::HashSet::new();

    for op in &plan.ops {
        let stmt_idx = op.source_stmt();
        if !seen_stmts.contains(&stmt_idx) {
            seen_stmts.insert(stmt_idx);
            // Find the corresponding source line
            // This is a simplification - in practice we'd need to track statement spans
            if stmt_idx < source_lines.len() {
                reordered_lines.push(source_lines[stmt_idx].to_string());
            }
        }
    }

    // Add any lines that weren't part of ops (comments, etc.)
    for (idx, line) in source_lines.iter().enumerate() {
        if !seen_stmts.contains(&idx) {
            // Insert at appropriate position (for now, append)
            if !line.trim().is_empty() && !line.trim().starts_with(';') {
                // Skip - this should have been in ops
            } else {
                reordered_lines.push(line.to_string());
            }
        }
    }

    let new_source = reordered_lines.join("\n");

    // Replace entire document
    let edit = TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: source_lines.len() as u32,
                character: 0,
            },
        },
        new_text: new_source,
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    Some(CodeAction {
        title: "Reorder statements for dependency resolution".to_string(),
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(false),
        disabled: None,
        data: None,
    })
}

/// Create a code action from a diagnostic's suggested fix
fn create_fix_action(
    diag: &ob_poc::dsl_v2::diagnostics::Diagnostic,
    fix: &SuggestedFix,
    uri: &Url,
    _source: &str,
) -> Option<CodeAction> {
    let range = Range {
        start: Position {
            line: fix.span.start_line.saturating_sub(1),
            character: fix.span.start_col.saturating_sub(1),
        },
        end: Position {
            line: fix.span.end_line.saturating_sub(1),
            character: fix.span.end_col.saturating_sub(1),
        },
    };

    let edit = TextEdit {
        range,
        new_text: fix.replacement.clone(),
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    // Determine if this is a preferred fix
    let is_preferred = matches!(
        diag.code,
        DiagnosticCode::MissingProducer | DiagnosticCode::UndefinedSymbol
    );

    Some(CodeAction {
        title: fix.description.clone(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(is_preferred),
        disabled: None,
        data: None,
    })
}

/// Create a code action from a semantic diagnostic suggestion (entity resolution)
///
/// When an entity reference can't be resolved but similar entities exist,
/// this creates quick fix actions like "Replace with 'John Smith'"
fn create_suggestion_action(
    diag: &SemanticDiagnostic,
    suggestion: &Suggestion,
    uri: &Url,
    _source: &str,
) -> Option<CodeAction> {
    // Use the suggestion's span if provided, otherwise use the diagnostic span
    let span = suggestion.replace_span.or(Some(diag.span))?;

    // Convert to LSP range (0-indexed)
    // SourceSpan has: line (1-based), column (0-based), offset, length
    let start_line = span.line.saturating_sub(1); // Convert to 0-based
    let start_char = span.column;

    // For end position, we need to calculate based on length
    // This is simplified - assumes the replacement is on a single line
    let end_char = start_char + span.length;

    let range = Range {
        start: Position {
            line: start_line,
            character: start_char,
        },
        end: Position {
            line: start_line, // Same line (simplified)
            character: end_char,
        },
    };

    // For entity references, we need to replace the quoted string value
    // The replacement should be the entity name/value
    let edit = TextEdit {
        range,
        new_text: format!("\"{}\"", suggestion.replacement),
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    // First suggestion is preferred (highest confidence)
    let is_preferred = suggestion.confidence > 0.8;

    // Format title with confidence
    let title = format!(
        "Replace with '{}' ({:.0}% match)",
        suggestion.replacement,
        suggestion.confidence * 100.0
    );

    Some(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(is_preferred),
        disabled: None,
        data: None,
    })
}
