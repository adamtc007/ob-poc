//! Document symbols handler.

use tower_lsp::lsp_types::*;

use crate::analysis::DocumentState;
use crate::analysis::document::ExprKind;

/// Get document symbols (outline).
#[allow(deprecated)]
pub fn get_document_symbols(doc: &DocumentState) -> Vec<SymbolInformation> {
    let mut symbols = Vec::new();

    for expr in &doc.expressions {
        if let ExprKind::Call { verb_name, verb_range: _, args } = &expr.kind {
            // Add the verb call as a symbol
            let mut name = verb_name.clone();

            // Try to add a meaningful identifier (e.g., :cbu-name value)
            for arg in args {
                if arg.keyword == ":cbu-name" || arg.keyword == ":name" || arg.keyword == ":first-name" {
                    if let Some(ref val) = arg.value {
                        if let ExprKind::String { value } = &val.kind {
                            name = format!("{} \"{}\"", verb_name, value);
                            break;
                        }
                    }
                }
            }

            // Check for :as @symbol
            for arg in args {
                if arg.keyword == ":as" {
                    if let Some(ref val) = arg.value {
                        if let ExprKind::SymbolRef { name: sym_name } = &val.kind {
                            name = format!("{} -> @{}", name, sym_name);
                            break;
                        }
                    }
                }
            }

            symbols.push(SymbolInformation {
                name,
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: Url::parse("file:///").unwrap(), // Will be set by caller
                    range: expr.range,
                },
                container_name: Some(verb_name.split('.').next().unwrap_or("dsl").to_string()),
            });
        }
    }

    // Add symbol definitions
    for def in &doc.symbol_defs {
        symbols.push(SymbolInformation {
            name: format!("@{}", def.name),
            kind: SymbolKind::VARIABLE,
            tags: None,
            deprecated: None,
            location: Location {
                uri: Url::parse("file:///").unwrap(),
                range: def.range,
            },
            container_name: Some(def.verb_name.clone()),
        });
    }

    symbols
}
