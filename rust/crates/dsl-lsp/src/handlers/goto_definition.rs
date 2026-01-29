//! Go-to-definition and find-references handler.

#![allow(dead_code)] // Public API - functions may be used by LSP server

use tower_lsp::lsp_types::*;

use crate::analysis::{DocumentState, SymbolTable};

/// Get definition location for symbol at position.
///
/// The `uri` parameter is the document URI, passed by the caller
/// so we can build proper Location objects.
pub fn get_definition(
    doc: &DocumentState,
    position: Position,
    symbols: &SymbolTable,
) -> Option<GotoDefinitionResponse> {
    let line = doc.get_line(position.line)?;
    let col = position.character as usize;

    // Find word at cursor
    let word = find_word_at_position(line, col)?;

    // Check if it's a symbol reference
    if let Some(symbol_name) = word.strip_prefix('@') {
        // Look up in symbol table (cross-document)
        if let Some(info) = symbols.get(symbol_name) {
            return Some(GotoDefinitionResponse::Scalar(info.definition.clone()));
        }

        // Fall back to document-local symbols
        // Note: URI is placeholder - server must fix it
        if let Some(def) = doc.get_symbol_def(symbol_name) {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: Url::parse("file:///placeholder").unwrap(),
                range: def.range,
            }));
        }
    }

    None
}

/// Get definition with proper URI (called from server with document URI).
pub fn get_definition_with_uri(
    doc: &DocumentState,
    position: Position,
    symbols: &SymbolTable,
    uri: &Url,
) -> Option<GotoDefinitionResponse> {
    let line = doc.get_line(position.line)?;
    let col = position.character as usize;

    // Find word at cursor
    let word = find_word_at_position(line, col)?;

    // Check if it's a symbol reference
    if let Some(symbol_name) = word.strip_prefix('@') {
        // Look up in symbol table (cross-document)
        if let Some(info) = symbols.get(symbol_name) {
            return Some(GotoDefinitionResponse::Scalar(info.definition.clone()));
        }

        // Fall back to document-local symbols
        if let Some(def) = doc.get_symbol_def(symbol_name) {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range: def.range,
            }));
        }
    }

    None
}

/// Get all references to symbol at position.
pub fn get_references(
    doc: &DocumentState,
    position: Position,
    symbols: &SymbolTable,
) -> Option<Vec<Location>> {
    let line = doc.get_line(position.line)?;
    let col = position.character as usize;

    // Find word at cursor
    let word = find_word_at_position(line, col)?;

    // Check if it's a symbol reference or definition
    let symbol_name = word.strip_prefix('@')?;

    // Look up in symbol table (cross-document)
    if let Some(info) = symbols.get(symbol_name) {
        let mut locations = vec![info.definition.clone()];
        locations.extend(info.references.iter().cloned());
        return Some(locations);
    }

    None
}

/// Get all references with proper URI for document-local fallback.
pub fn get_references_with_uri(
    doc: &DocumentState,
    position: Position,
    symbols: &SymbolTable,
    uri: &Url,
) -> Option<Vec<Location>> {
    let line = doc.get_line(position.line)?;
    let col = position.character as usize;

    // Find word at cursor
    let word = find_word_at_position(line, col)?;

    // Check if it's a symbol reference or definition
    let symbol_name = word.strip_prefix('@')?;

    // Look up in symbol table (cross-document)
    if let Some(info) = symbols.get(symbol_name) {
        let mut locations = vec![info.definition.clone()];
        locations.extend(info.references.iter().cloned());
        return Some(locations);
    }

    // Fall back to document-local
    let mut locations = Vec::new();

    // Add definition
    if let Some(def) = doc.get_symbol_def(symbol_name) {
        locations.push(Location {
            uri: uri.clone(),
            range: def.range,
        });
    }

    // Add references
    for sym_ref in &doc.symbol_refs {
        if sym_ref.name == symbol_name {
            locations.push(Location {
                uri: uri.clone(),
                range: sym_ref.range,
            });
        }
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

/// Find the word at a position in a line.
fn find_word_at_position(line: &str, col: usize) -> Option<String> {
    if col > line.len() {
        return None;
    }

    let chars: Vec<char> = line.chars().collect();

    // Find word boundaries
    let mut start = col;
    while start > 0 {
        let c = chars.get(start - 1)?;
        if !is_word_char(*c) {
            break;
        }
        start -= 1;
    }

    let mut end = col;
    while end < chars.len() {
        let c = chars.get(end)?;
        if !is_word_char(*c) {
            break;
        }
        end += 1;
    }

    if start == end {
        return None;
    }

    Some(chars[start..end].iter().collect())
}

/// Check if a character is part of a word.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ':' || c == '@'
}
