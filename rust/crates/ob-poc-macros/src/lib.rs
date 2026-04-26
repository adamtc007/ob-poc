//! Procedural macros for ob-poc
//!
//! This crate provides:
//! - `#[derive(IdType)]` — UUID-backed ID newtype boilerplate.
//!
//! # Phase 5c-migrate slice #80 note
//!
//! `#[register_custom_op]` was deleted altogether once every plugin op
//! had migrated to `sem_os_postgres::ops::SemOsVerbOp`; the sibling
//! `dsl-runtime-macros` crate that briefly owned it was removed in the
//! same slice.

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
