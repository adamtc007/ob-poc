#![allow(dead_code)]
//! Symbol table for tracking @symbol definitions across documents.

use std::collections::HashMap;
use tower_lsp::lsp_types::{Location, Range, Url};

use super::document::DocumentState;

/// Symbol information.
#[derive(Debug, Clone)]
pub(crate) struct SymbolInfo {
    /// Where the symbol is defined
    pub(crate) definition: Location,
    /// What verb defined it
    pub(crate) defined_by: String,
    /// What type of ID it holds (e.g., "CbuId", "EntityId")
    pub(crate) id_type: String,
    /// All references to this symbol
    pub(crate) references: Vec<Location>,
}

/// Cross-document symbol table.
#[derive(Debug, Clone, Default)]
pub(crate) struct SymbolTable {
    /// Symbols by name
    symbols: HashMap<String, SymbolInfo>,
    /// Symbols by document URI (for cleanup on close)
    by_document: HashMap<Url, Vec<String>>,
}

impl SymbolTable {
    /// Create a new empty symbol table.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Merge symbols from a document into the table.
    pub(crate) fn merge_from_document(&mut self, uri: &Url, doc: &DocumentState) {
        // Clear existing symbols from this document
        self.remove_document(uri);

        // Add new symbol definitions
        let mut doc_symbols = Vec::new();
        for def in &doc.symbol_defs {
            let info = SymbolInfo {
                definition: Location {
                    uri: uri.clone(),
                    range: def.range,
                },
                defined_by: def.defined_by.clone(),
                id_type: def.id_type.clone(),
                references: Vec::new(),
            };
            self.symbols.insert(def.name.clone(), info);
            doc_symbols.push(def.name.clone());
        }

        // Add references
        for sym_ref in &doc.symbol_refs {
            if let Some(info) = self.symbols.get_mut(&sym_ref.name) {
                info.references.push(Location {
                    uri: uri.clone(),
                    range: sym_ref.range,
                });
            }
        }

        self.by_document.insert(uri.clone(), doc_symbols);
    }

    /// Remove all symbols owned by a document.
    pub(crate) fn remove_document(&mut self, uri: &Url) {
        if let Some(old_symbols) = self.by_document.remove(uri) {
            for name in old_symbols {
                self.symbols.remove(&name);
            }
        }
    }

    /// Get symbol info by name.
    pub(crate) fn get(&self, name: &str) -> Option<&SymbolInfo> {
        self.symbols.get(name)
    }

    /// Get all symbol names.
    pub(crate) fn all_names(&self) -> Vec<&str> {
        self.symbols.keys().map(|s| s.as_str()).collect()
    }

    /// Get all symbols.
    pub(crate) fn all(&self) -> impl Iterator<Item = (&str, &SymbolInfo)> {
        self.symbols.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Find symbol at location.
    pub(crate) fn find_at_location(&self, uri: &Url, range: Range) -> Option<(&str, &SymbolInfo)> {
        for (name, info) in &self.symbols {
            // Check definition
            if info.definition.uri == *uri && ranges_overlap(&info.definition.range, &range) {
                return Some((name.as_str(), info));
            }
            // Check references
            for ref_loc in &info.references {
                if ref_loc.uri == *uri && ranges_overlap(&ref_loc.range, &range) {
                    return Some((name.as_str(), info));
                }
            }
        }
        None
    }
}

/// Infer the ID type from the verb that defined the symbol.
fn infer_id_type(verb_name: &str) -> String {
    match verb_name {
        "cbu.ensure" | "cbu.create" => "CbuId".to_string(),
        "entity.create-limited-company"
        | "entity.create-proper-person"
        | "entity.create-partnership"
        | "entity.create-trust" => "EntityId".to_string(),
        "investigation.create" => "InvestigationId".to_string(),
        "document.request" => "DocumentRequestId".to_string(),
        "screening.pep" | "screening.sanctions" => "ScreeningId".to_string(),
        "decision.record" => "DecisionId".to_string(),
        _ => "Unknown".to_string(),
    }
}

/// Check if two ranges overlap.
fn ranges_overlap(a: &Range, b: &Range) -> bool {
    !(a.end.line < b.start.line
        || (a.end.line == b.start.line && a.end.character < b.start.character)
        || b.end.line < a.start.line
        || (b.end.line == a.start.line && b.end.character < a.start.character))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::document::SymbolDef;
    use tower_lsp::lsp_types::Position;

    fn range(start: u32, end: u32) -> Range {
        Range {
            start: Position {
                line: 0,
                character: start,
            },
            end: Position {
                line: 0,
                character: end,
            },
        }
    }

    #[test]
    fn remove_document_removes_owned_symbols() {
        let uri = Url::parse("file:///a.dsl").unwrap();
        let mut doc = DocumentState::new(String::new());
        doc.symbol_defs.push(SymbolDef {
            name: "fund".to_string(),
            range: range(0, 5),
            defined_by: "cbu.create".to_string(),
            id_type: "cbu".to_string(),
        });

        let mut table = SymbolTable::new();
        table.merge_from_document(&uri, &doc);
        assert!(table.get("fund").is_some());

        table.remove_document(&uri);
        assert!(table.get("fund").is_none());
    }
}
