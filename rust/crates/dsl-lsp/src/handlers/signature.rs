//! Signature help handler.

use tower_lsp::lsp_types::*;

use crate::analysis::DocumentState;


use ob_poc::forth_engine::schema::registry::VERB_REGISTRY;
use ob_poc::forth_engine::schema::types::RequiredRule;

/// Get signature help at position.
pub fn get_signature_help(doc: &DocumentState, position: Position) -> Option<SignatureHelp> {
    // Find the enclosing call
    let (verb_name, args) = doc.find_call_at_position(position)?;

    // Get verb definition
    let verb = VERB_REGISTRY.get(verb_name)?;

    // Build signature
    let mut label_parts = vec![format!("({}", verb.name)];
    let mut parameters = Vec::new();

    for arg in verb.args {
        let required = matches!(arg.required, RequiredRule::Always);
        let param_label = if required {
            format!("{} <{}>", arg.name, arg.sem_type.type_name())
        } else {
            format!("[{} <{}>]", arg.name, arg.sem_type.type_name())
        };

        let param_start = label_parts.join(" ").len() + 1;
        label_parts.push(param_label.clone());
        let param_end = label_parts.join(" ").len();

        parameters.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([param_start as u32, param_end as u32]),
            documentation: Some(Documentation::String(arg.description.to_string())),
        });
    }

    label_parts.push(")".to_string());
    let label = label_parts.join(" ");

    // Determine active parameter based on cursor position
    let active_parameter = determine_active_parameter(doc, position, args.len());

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: verb.description.to_string(),
            })),
            parameters: Some(parameters),
            active_parameter: Some(active_parameter),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_parameter),
    })
}

/// Determine which parameter is active based on cursor position.
fn determine_active_parameter(
    _doc: &DocumentState,
    _position: Position,
    num_provided_args: usize,
) -> u32 {
    // Simple heuristic: use the number of provided args
    // In a more sophisticated implementation, we'd track which keyword
    // the cursor is currently after
    num_provided_args as u32
}
