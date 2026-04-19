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
//! # Expansion note — `crate::` reference
//!
//! The expansion emits `crate::domain_ops::CustomOpFactory` — `crate::`
//! resolves relative to the caller's crate, so ops in `ob-poc` continue
//! to see the trait + factory in `crate::domain_ops` unchanged. A future
//! slice (once legacy `execute()` is dissolved and the trait itself
//! relocates into `dsl-runtime`) will rewrite the expansion to absolute
//! `::dsl_runtime::*` paths.

use proc_macro::TokenStream;

mod register_op;

/// Auto-register a custom operation with the registry.
///
/// Apply to unit structs that implement `CustomOperation`.
///
/// **Current coupling:** the macro expansion references
/// `crate::domain_ops::CustomOperation` and
/// `crate::domain_ops::CustomOpFactory` — both paths must exist in the
/// calling crate (today: `ob-poc`). See the crate-level note above for
/// the future-slice migration to absolute paths.
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
