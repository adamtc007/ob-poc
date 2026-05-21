//! dsl-ast: Typed AST nodes for the unified DSL v0.1.
//!
//! Provides `AtomBag`, which classifies raw atoms from `dsl-parser` into
//! typed `TypedAtom` values using the kind taxonomy from `dsl-atoms`.
//!
//! Full per-kind slot extraction and type checking are Tranche 5 work.

pub mod atom_bag;

pub use atom_bag::{AtomBag, AtomIndex, TypedAtom};
