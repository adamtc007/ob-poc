//! Code actions handler for the DSL Language Server.
//!
//! Provides quick fixes and refactoring suggestions:
//! - Implicit create actions for undefined symbols
//! - Reorder statements for dependency resolution
//! - Entity suggestion quick fixes for unresolved refs

use tower_lsp::lsp_types::*;

use crate::encoding::{span_to_range, PositionEncoding};
use dsl_analysis::planning_facade::{PlanningOutput, SyntheticStep as PlanningSyntheticStep};
use dsl_analysis::validation::{Diagnostic as SemanticDiagnostic, Suggestion};
use dsl_core::ast::Statement;
use dsl_core::diagnostics::{DiagnosticCode, SuggestedFix};

/// Generate code actions from planning output and semantic diagnostics
///
/// This produces:
/// - Quick fixes for implicit creates (undefined symbols)
/// - Refactoring actions for reordering
/// - Entity suggestion quick fixes (e.g., "Did you mean 'John Smith'?")
pub(crate) fn get_code_actions(
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
    step: &PlanningSyntheticStep,
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

    // Reordering must operate on parser spans, not line numbers. Comments do
    // not have spans in the current AST, so avoid emitting edits that might
    // detach comments from the statement they describe.
    if planning_output
        .program
        .statements
        .iter()
        .any(|stmt| matches!(stmt, Statement::Comment(_)))
    {
        return None;
    }

    // Build reordered source from verb-call byte spans.
    let mut reordered_lines: Vec<String> = Vec::new();
    let mut seen_stmts = std::collections::HashSet::new();

    for op in &plan.ops {
        let stmt_idx = op.source_stmt();
        if !seen_stmts.contains(&stmt_idx) {
            seen_stmts.insert(stmt_idx);
            let stmt = planning_output.program.statements.get(stmt_idx)?;
            let Statement::VerbCall(verb_call) = stmt else {
                return None;
            };
            let stmt_source = source.get(verb_call.span.start..verb_call.span.end)?;
            reordered_lines.push(stmt_source.to_string());
        }
    }

    if seen_stmts.len() != planning_output.program.statements.len() {
        return None;
    }

    let new_source = reordered_lines.join("\n");

    // Replace entire document
    let edit = TextEdit {
        range: span_to_range(0, source.len(), source, PositionEncoding::Utf16),
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
    diag: &dsl_core::diagnostics::Diagnostic,
    fix: &SuggestedFix,
    uri: &Url,
    _source: &str,
) -> Option<CodeAction> {
    let start = line_col_to_offset(_source, fix.span.start_line, fix.span.start_col)?;
    let end = line_col_to_offset(_source, fix.span.end_line, fix.span.end_col)?;
    let range = span_to_range(start, end, _source, PositionEncoding::Utf16);

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

    let start = span.offset as usize;
    let end = start.saturating_add(span.length as usize);
    let range = span_to_range(start, end, _source, PositionEncoding::Utf16);

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
        (suggestion.confidence * 100.0).min(100.0)
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

fn line_col_to_offset(source: &str, line: u32, col: u32) -> Option<usize> {
    let target_line = line.saturating_sub(1) as usize;
    let target_col = col.saturating_sub(1) as usize;
    let mut current_line = 0usize;
    let mut line_start = 0usize;

    for (offset, ch) in source.char_indices() {
        if current_line == target_line {
            let line_end = source[offset..]
                .find('\n')
                .map(|pos| offset + pos)
                .unwrap_or(source.len());
            let line_text = &source[line_start..line_end];
            let byte_col = line_text
                .char_indices()
                .nth(target_col)
                .map(|(idx, _)| idx)
                .unwrap_or(line_text.len());
            return Some(line_start + byte_col);
        }

        if ch == '\n' {
            current_line += 1;
            line_start = offset + 1;
        }
    }

    if current_line == target_line {
        let line_text = &source[line_start..];
        let byte_col = line_text
            .char_indices()
            .nth(target_col)
            .map(|(idx, _)| idx)
            .unwrap_or(line_text.len());
        Some(line_start + byte_col)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_analysis::validation::{
        Diagnostic as SemanticDiagnostic, DiagnosticCode as SemanticDiagnosticCode, Severity,
        SourceSpan, Suggestion,
    };

    #[test]
    fn suggestion_action_uses_utf16_range_from_byte_span() {
        let source = "(test.verb :name \"🎉 Allianz\")";
        let start = source.find("Allianz").unwrap();
        let diag = SemanticDiagnostic {
            severity: Severity::Error,
            span: SourceSpan {
                line: 1,
                column: 0,
                offset: start as u32,
                length: "Allianz".len() as u32,
            },
            code: SemanticDiagnosticCode::InvalidValue,
            message: "unknown entity".to_string(),
            suggestions: vec![],
        };
        let suggestion = Suggestion::new("did you mean", "Allianz SE", 0.95);
        let uri = Url::parse("file:///test.dsl").unwrap();

        let action = create_suggestion_action(&diag, &suggestion, &uri, source).unwrap();
        let edit = action.edit.unwrap().changes.unwrap().remove(&uri).unwrap();

        assert_eq!(
            edit[0].range.start.character,
            source[..start].encode_utf16().count() as u32
        );
        assert_eq!(
            edit[0].range.end.character,
            source[..start + "Allianz".len()].encode_utf16().count() as u32
        );
    }
}
