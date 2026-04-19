//! Procedural macros for the DSL runtime data plane.
//!
//! # Phase 2c state
//!
//! Per `docs/todo/three-plane-architecture-implementation-plan-v0.1.md`
//! §3 Phase 2, this crate owns the `#[register_custom_op]` attribute
//! macro. It was moved here from `ob-poc-macros` so the macro is
//! co-located with its runtime types (the trait + factory it emits
//! impls against).
//!
//! # Expansion note — absolute `::dsl_runtime` paths
//!
//! The expansion emits `::dsl_runtime::CustomOperation` and
//! `::dsl_runtime::CustomOpFactory`. The trait + factory moved to the
//! `dsl-runtime` crate in Phase 2.5 Slice G; every caller crate that
//! invokes `#[register_custom_op]` must have `dsl-runtime` in scope.

use proc_macro::TokenStream;

mod register_op;

/// Auto-register a custom operation with the registry.
///
/// Apply to unit structs that implement `dsl_runtime::CustomOperation`.
///
/// The macro expansion references `::dsl_runtime::CustomOperation` +
/// `::dsl_runtime::CustomOpFactory` + `::inventory` by absolute path,
/// so the caller crate only needs `dsl-runtime` (and `inventory`) in its
/// dependency graph.
///
/// # Example
///
/// ```ignore
/// #[register_custom_op]
/// pub struct MyOp;
///
/// impl CustomOperation for MyOp {
///     fn domain(&self) -> &'static str { "my" }
///     fn verb(&self) -> &'static str { "op" }
///     fn rationale(&self) -> &'static str { "Custom logic required" }
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn register_custom_op(_attr: TokenStream, input: TokenStream) -> TokenStream {
    register_op::register_custom_op_impl(input)
}
