//! Common Taxonomy Model
//!
//! Generic taxonomy pattern for domains following the three-tier structure:
//! `Type → Operation → Resource`
//!
//! This module provides:
//! - `TaxonomyDomain` trait for domain-specific metadata
//! - `TaxonomyOps<D>` generic operations that work for any domain
//! - Concrete implementations for Product and Instrument domains
//!
//! # Philosophy
//!
//! "Think in bits and bytes / structures - then pivot functionality on metadata"
//!
//! All domains share the same structural pattern:
//! ```text
//! Domain Type ──(M:N)──► Operation ──(M:N)──► Resource Type
//!                                                  │
//!                                       CBU Instance Table
//! ```
//!
//! The only differences are metadata (table names, column names).

mod domain;
mod instrument;
mod ops;
mod product;

pub use domain::{TaxonomyDomain, TaxonomyMetadata};
pub use instrument::InstrumentDomain;
pub use ops::{Discovery, Gap, ProvisionArgs, ProvisionResult, TaxonomyOps};
pub use product::ProductDomain;
