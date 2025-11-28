//! Signature help handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::DocumentState;

use ob_poc::dsl_v2::find_verb;

/// Get signature help at position.
pub fn get_signature_help(doc: &DocumentState, position: Position) -> Option<SignatureHelp> {
    // Find which verb call we're in
    for expr in &doc.expressions {
        if let crate::analysis::document::ExprKind::Call {
            verb_name,
            verb_range: _,
            args: _,
        } = &expr.kind
        {
            if position_in_range(position, &expr.range) {
                let parts: Vec<&str> = verb_name.split('.').collect();
                if parts.len() != 2 {
                    continue;
                }

                let verb = find_verb(parts[0], parts[1])?;

                // Build signature
                let mut params = Vec::new();

                for arg in verb.required_args {
                    params.push(ParameterInformation {
                        label: ParameterLabel::Simple(format!(":{}", arg)),
                        documentation: Some(Documentation::String("(required)".to_string())),
                    });
                }

                for arg in verb.optional_args {
                    params.push(ParameterInformation {
                        label: ParameterLabel::Simple(format!(":{}", arg)),
                        documentation: Some(Documentation::String("(optional)".to_string())),
                    });
                }

                let signature = SignatureInformation {
                    label: format!("({} ...)", verb_name),
                    documentation: Some(Documentation::String(verb.description.to_string())),
                    parameters: Some(params),
                    active_parameter: None,
                };

                return Some(SignatureHelp {
                    signatures: vec![signature],
                    active_signature: Some(0),
                    active_parameter: None,
                });
            }
        }
    }

    None
}

fn position_in_range(position: Position, range: &Range) -> bool {
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
