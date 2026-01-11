//! GLEIF Source Loader
//!
//! Adapts the existing `GleifClient` to implement the `SourceLoader` trait.
//! GLEIF provides global LEI (Legal Entity Identifier) data and corporate hierarchies.
//!
//! # Coverage
//!
//! - **Jurisdictions:** Global ("*")
//! - **Key type:** LEI (20 alphanumeric characters)
//! - **Provides:** Entity, ParentChain
//! - **Does NOT provide:** ControlHolders (no shareholder data), Officers

mod loader;
mod normalize;

pub use loader::GleifLoader;
