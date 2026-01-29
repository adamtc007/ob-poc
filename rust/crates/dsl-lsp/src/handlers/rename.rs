//! Rename handler for the DSL Language Server.
//!
//! Supports renaming symbols (@name) across a document.

#![allow(dead_code)] // Public API - functions used by LSP server

use std::collections::HashMap;
use tower_lsp::lsp_types::*;

use crate::analysis::DocumentState;

/// Prepare rename - check if rename is valid at position.
pub fn prepare_rename(doc: &DocumentState, position: Position) -> Option<PrepareRenameResponse> {
    // Check if cursor is on a symbol definition
    for def in &doc.symbol_defs {
        if position_in_range(position, &def.range) {
            return Some(PrepareRenameResponse::Range(def.range));
        }
    }

    // Check if cursor is on a symbol reference
    for sym_ref in &doc.symbol_refs {
        if position_in_range(position, &sym_ref.range) {
            return Some(PrepareRenameResponse::Range(sym_ref.range));
        }
    }

    None
}

/// Execute rename - return workspace edit.
pub fn rename_symbol(
    doc: &DocumentState,
    position: Position,
    new_name: &str,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    // Find the symbol name at position
    let symbol_name = find_symbol_at_position(doc, position)?;

    // Clean up new name (remove @ prefix if user added it)
    let new_name = new_name.strip_prefix('@').unwrap_or(new_name);

    // Collect all edits
    let mut edits = Vec::new();

    // Edit the definition
    for def in &doc.symbol_defs {
        if def.name == symbol_name {
            edits.push(TextEdit {
                range: def.range,
                new_text: format!("@{}", new_name),
            });
        }
    }

    // Edit all references
    for sym_ref in &doc.symbol_refs {
        if sym_ref.name == symbol_name {
            edits.push(TextEdit {
                range: sym_ref.range,
                new_text: format!("@{}", new_name),
            });
        }
    }

    if edits.is_empty() {
        return None;
    }

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

/// Find the symbol name at a position.
fn find_symbol_at_position(doc: &DocumentState, position: Position) -> Option<String> {
    // Check definitions
    for def in &doc.symbol_defs {
        if position_in_range(position, &def.range) {
            return Some(def.name.clone());
        }
    }

    // Check references
    for sym_ref in &doc.symbol_refs {
        if position_in_range(position, &sym_ref.range) {
            return Some(sym_ref.name.clone());
        }
    }

    None
}

/// Check if position is within range.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::document::{SymbolDef, SymbolRef};

    fn make_doc_with_symbols() -> DocumentState {
        let text =
            "(cbu.create :name \"Test\" :as @my-cbu)\n(cbu.update :cbu-id @my-cbu)".to_string();
        let mut doc = DocumentState::new(text);

        // Add symbol definition
        doc.symbol_defs.push(SymbolDef {
            name: "my-cbu".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 29,
                },
                end: Position {
                    line: 0,
                    character: 36,
                },
            },
            defined_by: "cbu.create".to_string(),
            id_type: "cbu".to_string(),
        });

        // Add symbol reference
        doc.symbol_refs.push(SymbolRef {
            name: "my-cbu".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 20,
                },
                end: Position {
                    line: 1,
                    character: 27,
                },
            },
        });

        doc
    }

    #[test]
    fn test_prepare_rename_on_definition() {
        let doc = make_doc_with_symbols();
        let result = prepare_rename(
            &doc,
            Position {
                line: 0,
                character: 32,
            },
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_prepare_rename_on_reference() {
        let doc = make_doc_with_symbols();
        let result = prepare_rename(
            &doc,
            Position {
                line: 1,
                character: 23,
            },
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_rename_symbol() {
        let doc = make_doc_with_symbols();
        let uri = Url::parse("file:///test.dsl").unwrap();
        let result = rename_symbol(
            &doc,
            Position {
                line: 0,
                character: 32,
            },
            "new-cbu",
            &uri,
        );

        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();

        // Should have 2 edits: definition and reference
        assert_eq!(edits.len(), 2);

        for edit in edits {
            assert_eq!(edit.new_text, "@new-cbu");
        }
    }

    #[test]
    fn test_rename_strips_at_prefix() {
        let doc = make_doc_with_symbols();
        let uri = Url::parse("file:///test.dsl").unwrap();
        // User types "@new-cbu" with the @ prefix
        let result = rename_symbol(
            &doc,
            Position {
                line: 0,
                character: 32,
            },
            "@new-cbu",
            &uri,
        );

        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();

        // Should still produce "@new-cbu" not "@@new-cbu"
        for edit in edits {
            assert_eq!(edit.new_text, "@new-cbu");
        }
    }
}
