//! Procedural macros for ob-poc
//!
//! This crate provides two macros:
//! - `#[register_custom_op]` - Auto-register custom operations with the registry
//! - `#[derive(IdType)]` - Generate boilerplate for UUID-backed ID newtypes

use proc_macro::TokenStream;

mod id_type;
mod register_op;

/// Auto-register a custom operation with the registry.
///
/// Apply to unit structs that implement `CustomOperation`.
///
/// **Important:** All ops must live in the main crate (uses `crate::domain_ops` path).
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

/// Derive macro for UUID-backed ID newtypes.
///
/// Generates implementations for: Clone, Copy, Debug, Display, FromStr,
/// PartialEq, Eq, Hash, Serialize, Deserialize, and (with `database` feature)
/// SQLx traits.
///
/// **Important:** Do NOT also derive Clone, Copy, Debug, PartialEq, Eq, Hash,
/// Serialize, or Deserialize — IdType generates all of these.
///
/// # Attributes
///
/// - `#[id(prefix = "...")]` - Optional prefix for Display/FromStr (e.g., "req" → "req_<uuid>")
/// - `#[id(new_v4)]` - Generate `::new()` and `Default` implementations
///
/// # Example
///
/// ```ignore
/// #[derive(IdType)]
/// #[id(prefix = "req", new_v4)]
/// pub struct RequirementId(Uuid);
/// ```
#[proc_macro_derive(IdType, attributes(id))]
pub fn derive_id_type(input: TokenStream) -> TokenStream {
    id_type::derive_id_type_impl(input)
}
