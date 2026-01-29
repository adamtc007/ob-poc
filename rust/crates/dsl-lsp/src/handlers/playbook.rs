//! Playbook file analysis and diagnostics

use playbook_core::parse_playbook;
use playbook_lower::{lower_playbook, SlotState};
use tower_lsp::lsp_types::*;

/// Analyze a playbook YAML file and return diagnostics
pub async fn analyze_playbook(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Parse the playbook
    let output = match parse_playbook(source) {
        Ok(o) => o,
        Err(e) => {
            diagnostics.push(Diagnostic {
                range: Range::default(),
                severity: Some(DiagnosticSeverity::ERROR),
                message: e.to_string(),
                source: Some("playbook".to_string()),
                ..Default::default()
            });
            return diagnostics;
        }
    };

    // Lower to DSL (validates structure)
    let slots = SlotState::new();
    let result = lower_playbook(&output.spec, &slots);

    // Report missing slots as warnings
    for m in &result.missing_slots {
        if let Some(span) = output.source_map.verb_span(m.step_index) {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: span.line - 1,
                        character: span.column,
                    },
                    end: Position {
                        line: span.line - 1,
                        character: span.column + span.length,
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                message: format!("Missing slot: {}", m.name),
                source: Some("playbook".to_string()),
                ..Default::default()
            });
        }
    }

    // TODO: Validate verbs exist in registry
    // TODO: Run DAG validation on lowered DSL

    diagnostics
}
