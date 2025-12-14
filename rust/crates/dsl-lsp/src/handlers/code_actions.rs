//! Code actions handler for the DSL Language Server.
//!
//! Provides quick fixes and refactoring suggestions:
//! - Implicit create actions for undefined symbols
//! - Reorder statements for dependency resolution

use tower_lsp::lsp_types::*;

use ob_poc::dsl_v2::diagnostics::{DiagnosticCode, SuggestedFix};
use ob_poc::dsl_v2::planning_facade::{PlanningOutput, SyntheticStep};

/// Generate code actions from planning output
///
/// This produces:
/// - Quick fixes for implicit creates (undefined symbols)
/// - Refactoring actions for reordering
pub fn get_code_actions(
    planning_output: &PlanningOutput,
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
