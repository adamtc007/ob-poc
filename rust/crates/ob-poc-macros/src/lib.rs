//! Procedural macros for ob-poc
//!
//! This crate provides:
//! - `#[derive(IdType)]` — UUID-backed ID newtype boilerplate.
//!
//! # Phase 2c note
//!
//! `#[register_custom_op]` moved to `dsl-runtime-macros` per the three-plane
//! architecture plan. Import sites updated in the same slice:
//!
//! ```text
//! // before
//! use ob_poc_macros::register_custom_op;
//! // after
//! use dsl_runtime_macros::register_custom_op;
//! ```

use proc_macro::TokenStream;

mod id_type;

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
