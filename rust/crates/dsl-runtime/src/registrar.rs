//! `VerbRegistrar` — forward-looking verb-registration contract.
//!
//! Per `docs/todo/three-plane-architecture-v0.3.md` §13 Phase 2, the
//! `#[register_custom_op]` macro emits `VerbRegistrar` impls alongside
//! its existing `CustomOpFactory`/`inventory` wiring. Today the trait is
//! a scaffold: it declares the shape every plugin op metadata must carry
//! (domain, verb, rationale) at compile time, independent of the
//! runtime trait object the registry holds.
//!
//! # Phase 2c status
//!
//! **Scaffold only** — no consumers yet. The `ob-poc::domain_ops`
//! `CustomOperation` trait already provides the `domain()` / `verb()` /
//! `rationale()` methods via trait objects, and the inventory-driven
//! `CustomOpFactory` remains the concrete registration mechanism.
//! `VerbRegistrar` exists so future slices (Phase 5b Sequencer +
//! `CustomOperation` relocation into `dsl-runtime`) can wire static
//! metadata checks without retrofitting the trait shape.
//!
//! # Shape
//!
//! Each registered op implements `VerbRegistrar` — typically via the
//! `#[register_custom_op]` macro — to publish its compile-time metadata.
//! Runtime dispatch continues to flow through
//! [`crate::VerbExecutionPort::execute_verb`]; `VerbRegistrar` exists
//! purely for registration-time introspection and determinism harness
//! fixtures.
pub trait VerbRegistrar {
    /// Domain this op belongs to, e.g. `"cbu"`.
    fn domain() -> &'static str
    where
        Self: Sized;

    /// Verb name this op handles, e.g. `"create"`.
    fn verb() -> &'static str
    where
        Self: Sized;

    /// Human-readable rationale — why this op requires custom Rust.
    fn rationale() -> &'static str
    where
        Self: Sized;

    /// Fully-qualified verb name: `"<domain>.<verb>"`.
    fn fqn() -> String
    where
        Self: Sized,
    {
        format!("{}.{}", Self::domain(), Self::verb())
    }
}
