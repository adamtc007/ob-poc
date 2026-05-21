//! Raw (untyped) parse tree types for the unified DSL v0.1.
//!
//! The raw AST mirrors the S-expression surface syntax closely. Atoms carry
//! their kind string unclassified — classification happens in `dsl-ast` when
//! building the `AtomBag`.

use dsl_diagnostics::Span;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Value types
// ---------------------------------------------------------------------------

/// A single value node in the raw parse tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawValue {
    /// A nested atom `(kind [name] :slot value ...)`.
    Atom(RawAtom),
    /// A bracketed list of values `[v1 v2 ...]`.
    List(Vec<RawValue>),
    /// A braced map `{ :k1 v1 :k2 v2 ... }`.
    Map(Vec<(String, RawValue)>),
    /// An unquoted symbol (identifier).
    Symbol(String),
    /// A `pack-name/atom-name` qualified reference.
    QualifiedName { pack: String, atom: String },
    /// A double-quoted string literal.
    StringLit(String),
    /// An integer literal.
    IntLit(i64),
    /// A floating-point literal.
    FloatLit(f64),
    /// A boolean literal.
    BoolLit(bool),
    /// An `@symbol` slot reference (cross-atom binding).
    SlotRef(String),
    /// A `,symbol` template parameter substitution.
    TemplateSubst(String),
    /// A `,@symbol` template splice (list-valued parameter, expands in-place).
    TemplateSplice(String),
    /// A `$symbol` insertion marker (pre/post-node template position).
    InsertionMarker(String),
    /// `(for-each :var VAR :in LIST_PARAM body-atoms...)` — template loop form.
    ///
    /// Valid only inside a `(decision-pack :template [...])` slot.  At
    /// instantiation time the loop is unrolled: one copy of `body` is emitted
    /// per element of the list parameter named `list_param`, with
    /// `,VAR.field` accessors substituted from the element's fields.
    ForEach {
        /// Loop variable name (e.g. `"band"`).
        var: String,
        /// Parameter name to iterate over (e.g. `"bands"`).
        list_param: String,
        /// Atom bodies to repeat per element.
        body: Vec<RawValue>,
    },
}

// ---------------------------------------------------------------------------
// Atom
// ---------------------------------------------------------------------------

/// A raw atom as parsed from the S-expression source.
///
/// An atom has the surface form: `(kind [name] :slot value ...)`
///
/// - `kind` is always present (first symbol inside the parens).
/// - `name` is optional — present when the second token after `kind` is a
///   `Symbol` that does not start with `:`.
/// - `slots` is an ordered list of `(slot_name, value)` pairs following the
///   optional name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawAtom {
    /// The atom kind string, e.g. `"gateway"`, `"flow"`.
    pub kind: String,
    /// Optional atom name, e.g. `"activation-gate"`.
    pub name: Option<String>,
    /// Ordered slot pairs `(slot_name, value)`.
    pub slots: Vec<(String, RawValue)>,
    /// Source location of the opening paren, if available.
    pub span: Option<Span>,
}

// ---------------------------------------------------------------------------
// SourceFile
// ---------------------------------------------------------------------------

/// The top-level parse result: a sequence of top-level atoms.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub atoms: Vec<RawAtom>,
}
