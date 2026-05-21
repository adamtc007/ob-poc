//! Typed atom bag: classifies raw atoms and provides name-indexed access.
//!
//! `AtomBag` is the primary output of the `dsl-ast` crate. It is built from a
//! `SourceFile` produced by `dsl-parser` by classifying each atom's kind string
//! via `dsl-atoms::classify`. Full per-kind slot extraction is Tranche 5 work;
//! for now each `TypedAtom` carries the raw form plus its `AtomKindClass`.

use std::collections::HashMap;

use dsl_atoms::{classify, AtomKindClass, StructuralKind};
use dsl_diagnostics::{Diagnostic, DiagnosticBag, UNKNOWN_ATOM_KIND};
use dsl_parser::raw_ast::{RawAtom, SourceFile};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AtomIndex
// ---------------------------------------------------------------------------

/// A typed index into an `AtomBag`. Cheap to copy and compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AtomIndex(pub usize);

// ---------------------------------------------------------------------------
// TypedAtom
// ---------------------------------------------------------------------------

/// A raw atom paired with its classified kind.
///
/// Slot extraction is intentionally deferred to Tranche 5. Consumers that need
/// slot values should access `raw.slots` directly until typed accessors land.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedAtom {
    /// Classified atom kind (may be `UnknownStructural` for parse errors).
    pub kind_class: AtomKindClass,
    /// Optional atom name (mirrors `raw.name`).
    pub name: Option<String>,
    /// The raw atom from the parser.
    pub raw: RawAtom,
}

// ---------------------------------------------------------------------------
// AtomBag
// ---------------------------------------------------------------------------

/// A classified, name-indexed collection of atoms from a single source file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AtomBag {
    atoms: Vec<TypedAtom>,
    by_name: HashMap<String, AtomIndex>,
}

impl AtomBag {
    /// Build an `AtomBag` from a parsed `SourceFile`.
    ///
    /// Each atom's kind string is classified via [`classify`]. Duplicate atom
    /// names produce a `Warning` diagnostic; the second occurrence is still
    /// added to the bag but is NOT indexed by name (first-wins).
    pub fn from_source_file(source: SourceFile, diagnostics: &mut DiagnosticBag) -> Self {
        let mut atoms: Vec<TypedAtom> = Vec::with_capacity(source.atoms.len());
        let mut by_name: HashMap<String, AtomIndex> = HashMap::new();

        for raw in source.atoms {
            let kind_class = classify(&raw.kind);

            // Emit a diagnostic for unknown atom kinds
            if let AtomKindClass::UnknownStructural(ref s) = kind_class {
                diagnostics.push(
                    Diagnostic::error(format!("Unknown atom kind '{}'", s))
                        .with_code(UNKNOWN_ATOM_KIND),
                );
            }

            let name = raw.name.clone();
            let index = AtomIndex(atoms.len());

            let typed = TypedAtom {
                kind_class,
                name: name.clone(),
                raw,
            };

            atoms.push(typed);

            if let Some(ref n) = name {
                if by_name.contains_key(n.as_str()) {
                    diagnostics.push(Diagnostic::warning(format!(
                        "Duplicate atom name '{}'; first occurrence wins in name index",
                        n
                    )));
                } else {
                    by_name.insert(n.clone(), index);
                }
            }
        }

        Self { atoms, by_name }
    }

    /// Return the `TypedAtom` at the given index.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds (indices are always produced by the
    /// bag that issued them, so this should not occur in practice).
    pub fn get(&self, idx: AtomIndex) -> &TypedAtom {
        &self.atoms[idx.0]
    }

    /// Find an atom by name. Returns `None` if no atom with that name exists
    /// or if the name was a duplicate (first-wins; later duplicates are not
    /// indexed).
    pub fn find(&self, name: &str) -> Option<AtomIndex> {
        self.by_name.get(name).copied()
    }

    /// Iterate over all structural atoms (excludes declarative and unknown kinds).
    pub fn structural_atoms(&self) -> impl Iterator<Item = &TypedAtom> {
        self.atoms.iter().filter(|a| {
            matches!(a.kind_class, AtomKindClass::Structural(_))
        })
    }

    /// Iterate over all declarative atoms.
    pub fn declarative_atoms(&self) -> impl Iterator<Item = &TypedAtom> {
        self.atoms.iter().filter(|a| {
            matches!(a.kind_class, AtomKindClass::Declarative(_))
        })
    }

    /// Return all atoms with a specific structural kind.
    pub fn atoms_of_structural_kind(&self, kind: StructuralKind) -> Vec<&TypedAtom> {
        self.atoms
            .iter()
            .filter(|a| a.kind_class == AtomKindClass::Structural(kind.clone()))
            .collect()
    }

    /// Total number of atoms in the bag.
    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    /// Returns `true` if the bag contains no atoms.
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_atoms::DeclarativeKind;
    use dsl_parser::parse;

    const MINI_SOURCE: &str = r#"
        (gateway start-gate :kind exclusive)
        (provenance :author "test")
    "#;

    #[test]
    fn bag_from_mini_source() {
        let (sf, _) = parse(MINI_SOURCE);
        let mut diag = DiagnosticBag::new();
        let bag = AtomBag::from_source_file(sf, &mut diag);

        assert_eq!(bag.len(), 2);
        assert!(!diag.has_errors(), "unexpected errors: {:?}", diag.diagnostics);

        let structural: Vec<_> = bag.structural_atoms().collect();
        assert_eq!(structural.len(), 1);
        assert_eq!(structural[0].raw.kind, "gateway");

        let declarative: Vec<_> = bag.declarative_atoms().collect();
        assert_eq!(declarative.len(), 1);
        assert_eq!(declarative[0].raw.kind, "provenance");
    }

    #[test]
    fn find_by_name() {
        let (sf, _) = parse(MINI_SOURCE);
        let mut diag = DiagnosticBag::new();
        let bag = AtomBag::from_source_file(sf, &mut diag);

        let idx = bag.find("start-gate");
        assert!(idx.is_some(), "expected to find 'start-gate'");
        let atom = bag.get(idx.unwrap());
        assert_eq!(atom.raw.kind, "gateway");
    }

    #[test]
    fn atoms_of_structural_kind() {
        let src = "(gateway g1 :kind exclusive) (gateway g2 :kind parallel) (node n :label \"N\")";
        let (sf, _) = parse(src);
        let mut diag = DiagnosticBag::new();
        let bag = AtomBag::from_source_file(sf, &mut diag);

        let gateways = bag.atoms_of_structural_kind(StructuralKind::Gateway);
        assert_eq!(gateways.len(), 2);

        let nodes = bag.atoms_of_structural_kind(StructuralKind::Node);
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn duplicate_name_produces_diagnostic() {
        let src = "(node foo :x 1) (node foo :x 2)";
        let (sf, _) = parse(src);
        let mut diag = DiagnosticBag::new();
        let bag = AtomBag::from_source_file(sf, &mut diag);

        // Both atoms should be in the bag
        assert_eq!(bag.len(), 2);
        // But a warning should have been emitted
        assert!(!diag.has_errors(), "duplicate name should be a warning, not an error");
        assert_eq!(diag.warnings().count(), 1);
        // The name index points to the first occurrence
        let idx = bag.find("foo").unwrap();
        assert_eq!(bag.get(idx).raw.slots[0].0, "x");
        // Both atoms have slot x, first has value 1
        match &bag.get(idx).raw.slots[0].1 {
            dsl_parser::raw_ast::RawValue::IntLit(v) => assert_eq!(*v, 1),
            other => panic!("expected IntLit(1), got {:?}", other),
        }
    }

    #[test]
    fn unknown_kind_produces_error() {
        let src = "(flux-capacitor foo :speed 88)";
        let (sf, _) = parse(src);
        let mut diag = DiagnosticBag::new();
        let bag = AtomBag::from_source_file(sf, &mut diag);

        assert_eq!(bag.len(), 1);
        assert!(diag.has_errors(), "unknown kind should produce an error");
        let err = diag.errors().next().unwrap();
        assert_eq!(err.code.as_deref(), Some(UNKNOWN_ATOM_KIND));
    }

    #[test]
    fn declarative_kind_coverage() {
        let src = r#"
            (provenance :author "a")
            (governance-status :state active)
            (review-annotation :note "ok")
            (jurisdiction-tag :region EU)
        "#;
        let (sf, _) = parse(src);
        let mut diag = DiagnosticBag::new();
        let bag = AtomBag::from_source_file(sf, &mut diag);

        assert!(!diag.has_errors());
        let decl: Vec<_> = bag.declarative_atoms().collect();
        assert_eq!(decl.len(), 4);
        assert!(decl.iter().any(|a| a.kind_class == AtomKindClass::Declarative(DeclarativeKind::JurisdictionTag)));
    }
}
