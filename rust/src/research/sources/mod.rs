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
pub mod gleif;
pub mod normalized;
pub mod registry;
pub mod sec_edgar;
pub mod traits;

// Re-exports
pub use normalized::{
    EntityStatus, EntityType, HolderType, NormalizedAddress, NormalizedControlHolder,
    NormalizedEntity, NormalizedOfficer, NormalizedRelationship, OfficerRole, RelationshipType,
};
pub use registry::{SourceInfo, SourceRegistry};
pub use traits::{
    FetchControlHoldersOptions, FetchOfficersOptions, FetchOptions, FetchParentChainOptions,
    SearchCandidate, SearchOptions, SourceDataType, SourceLoader,
};

// Source loader implementations
pub use companies_house::CompaniesHouseLoader;
pub use gleif::GleifLoader;
pub use sec_edgar::SecEdgarLoader;
