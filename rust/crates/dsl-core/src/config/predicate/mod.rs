//! Machine-readable `green_when` predicate support.

pub mod ast;
pub mod parser;

pub use ast::{
    AttrName, AttrValue, CmpOp, CountOp, EntityKind, EntityQualifier, EntityRef, EntitySetRef,
    Predicate, RelationScope, State, StateSet, Validity,
};
pub use parser::{parse_green_when, ParseError};
