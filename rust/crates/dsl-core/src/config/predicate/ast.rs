//! Predicate AST for DAG `green_when` expressions.

use serde::{Deserialize, Serialize};

/// A machine-evaluable `green_when` predicate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Predicate {
    /// All child predicates must evaluate green.
    And(Vec<Predicate>),

    /// A singleton entity, relationship, row, or required child population exists.
    Exists { entity: EntityRef },

    /// A referenced entity's state is inside the allowed state set.
    StateIn {
        entity: EntityRef,
        state_set: StateSet,
    },

    /// An attribute comparison on a referenced entity.
    AttrCmp {
        entity: EntityRef,
        attr: AttrName,
        op: CmpOp,
        value: AttrValue,
    },

    /// Every member of a bounded or compiler-approved set satisfies a condition.
    Every {
        set: EntitySetRef,
        condition: Box<Predicate>,
    },

    /// No member of a set exists that satisfies a condition.
    NoneExists {
        set: EntitySetRef,
        condition: Box<Predicate>,
    },

    /// At least one member of a set satisfies a condition.
    AtLeastOne {
        set: EntitySetRef,
        condition: Box<Predicate>,
    },

    /// Count members of a set satisfying an optional condition.
    Count {
        set: EntitySetRef,
        condition: Option<Box<Predicate>>,
        op: CountOp,
        threshold: u64,
    },

    /// `obtained(X)`: exists plus validity, possibly delegated to X's own DAG.
    Obtained {
        entity: EntityRef,
        validity: Validity,
    },
}

/// A singleton entity or contextual entity reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityRef {
    /// The current DAG instance.
    This,

    /// A named child/entity in the current instance scope.
    Named(EntityKind),

    /// A named parent entity, e.g. `parent kyc_case`.
    Parent(EntityKind),

    /// A named entity with an explicit textual scope from the source predicate.
    Scoped {
        kind: EntityKind,
        scope: RelationScope,
    },
}

/// A set of entities targeted by a quantified predicate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySetRef {
    /// Entity kind named in the predicate.
    pub kind: EntityKind,
    /// Optional qualifier such as `required`.
    pub qualifier: Option<EntityQualifier>,
    /// Optional relation scope such as `for this UBO`.
    pub scope: Option<RelationScope>,
}

/// Qualifier attached to a set reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityQualifier {
    /// The authored predicate says the required population must be complete.
    Required,
}

/// Relationship or scope phrase preserved as structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationScope {
    /// Scoped to the current instance of the named kind.
    This(EntityKind),

    /// Scoped to a parent instance of the named kind.
    Parent(EntityKind),

    /// Scoped to rows attached to the current instance of the named kind.
    AttachedTo(EntityKind),
}

/// Validity rule for `obtained`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Validity {
    /// Valid when the entity's state is one of the listed states.
    StateIn(StateSet),

    /// Validity is delegated to the referenced entity's own DAG.
    DelegatedToEntityDag,
}

/// Allowed state names.
pub type StateSet = Vec<State>;

/// DAG/entity/slot kind name as authored in YAML.
pub type EntityKind = String;

/// State name as authored in YAML.
pub type State = String;

/// Attribute name as authored in YAML.
pub type AttrName = String;

/// Comparison operator for attributes and counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    /// Equality.
    Eq,
    /// Inequality.
    Ne,
    /// Less-than.
    Lt,
    /// Less-than or equal.
    Le,
    /// Greater-than.
    Gt,
    /// Greater-than or equal.
    Ge,
}

/// Count comparison operator.
pub type CountOp = CmpOp;

/// Right-hand side of an attribute comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttrValue {
    /// Quoted or textual string value.
    String(String),
    /// Numeric literal preserved as authored.
    Number(String),
    /// Symbolic value such as a threshold name or enum atom.
    Symbol(String),
    /// Boolean literal.
    Bool(bool),
}
