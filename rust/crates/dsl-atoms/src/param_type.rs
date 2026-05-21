//! Parameter type descriptors for atom slot declarations.
//!
//! Each slot in an atom definition has a `ParamType` that governs what
//! value shapes are valid during type-checking (Tranche 5+).

use serde::{Deserialize, Serialize};

/// The set of value types that an atom slot can be declared to accept.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParamType {
    /// A literal string value.
    String_,
    /// A symbolic identifier (unquoted atom name or keyword value).
    Symbol,
    /// A 64-bit signed integer literal.
    Integer,
    /// A boolean literal (`true` / `false`).
    Boolean,
    /// A reference to another atom by name (resolved to `AtomIndex` in Tranche 5).
    NodeRef,
    /// An inline condition expression.
    ConditionExpr,
    /// A reference to a named predicate atom.
    PredicateRef,
    /// A list of condition expressions.
    ListOfConditionExpr,
    /// A list of predicate references.
    ListOfPredicateRef,
    /// A list of node references.
    ListOfNodeRef,
    /// A reference to a decision atom.
    DecisionRef,
    /// A map of path → value entries (used for structured metadata slots).
    PathMap,
    /// A list of map objects, each with string keys and typed values.
    ///
    /// Used with `for-each` loops in pack templates.  Element fields are
    /// accessed inside the loop body via `,var.field` syntax.
    ListOfMap,
}
