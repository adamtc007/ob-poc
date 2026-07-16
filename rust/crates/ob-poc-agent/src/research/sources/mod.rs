//! Pluggable Research Source Loaders
//!
//! This module provides a trait-based abstraction for external data sources
//! (GLEIF, Companies House, SEC EDGAR, etc.) that normalize to our entity model.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │   GLEIF     │     │ Companies   │     │ SEC EDGAR   │
//! │   Loader    │     │   House     │     │   Loader    │
//! └──────┬──────┘     └──────┬──────┘     └──────┬──────┘
//!        │                   │                   │
//!        └───────────────────┼───────────────────┘
//!                            │
//!                            ▼
//!                  ┌─────────────────┐
//!                  │  SourceLoader   │  Trait
//!                  │  Trait          │
//!                  └────────┬────────┘
//!                           │
//!                           ▼
//!                  ┌─────────────────┐
//!                  │   Normalized    │  Structs
//!                  │   Structures    │
//!                  └────────┬────────┘
//!                           │
//!                           ▼
//!                  ┌─────────────────┐
//!                  │   Repository    │  DB writes
//!                  │   Functions     │
//!                  └─────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use ob_poc::research::sources::{SourceRegistry, SourceDataType};
//!
//! let registry = SourceRegistry::new();
//!
//! // Find sources for UK jurisdiction
//! let uk_sources = registry.find_for_jurisdiction("GB", SourceDataType::ControlHolders);
//!
//! // Search using a specific source
//! if let Some(ch) = registry.get("companies-house") {
//!     let results = ch.search("Acme Ltd", Some("GB")).await?;
//! }
//! ```

pub mod companies_house;
pub mod normalized;
pub mod registry;
pub mod sec_edgar;
pub mod traits;

// T11.1b (2026-07-12): the GLEIF loader stays in `ob-poc` — it wraps
// `GleifClient`, a real HTTP capability, and nothing in this crate's
// research module references `GleifLoader` directly (confirmed: no
// dynamic-registry wiring calls it by name). `ob-poc::research::sources`
// re-exports this module's public surface alongside its own local `gleif`
// submodule so `crate::research::sources::{traits,normalized,...}` callers
// (elsewhere in `ob-poc`) see one unified path, same as before this move.

// Re-exports

// Source loader implementations
pub use companies_house::CompaniesHouseLoader;
pub use sec_edgar::SecEdgarLoader;
