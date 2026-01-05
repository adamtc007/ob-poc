//! GLEIF (Global Legal Entity Identifier Foundation) integration
//!
//! This module provides:
//! - API types for GLEIF Level 1 (entity data) and Level 2 (relationships)
//! - Repository for persisting GLEIF data to our schema
//! - Client for fetching data from the GLEIF API
//! - Enrichment service for populating entities with GLEIF data
//!
//! # External API Resilience Pattern
//!
//! When deserializing external API responses (GLEIF, BODS, etc.), **never let
//! unknown enum variants crash the pipeline**. External APIs evolve - new codes,
//! statuses, and categories appear without notice.
//!
//! ## Anti-pattern (brittle):
//!
//! ```rust,ignore
//! #[derive(Deserialize)]
//! pub enum EntityLegalForm {
//!     #[serde(rename = "8888")]
//!     PublicLimitedCompany,
//!     #[serde(rename = "8889")]
//!     PrivateLimitedCompany,
//!     // GLEIF sends "9999" → deserialize fails → verb fails
//! }
//! ```
//!
//! ## Pattern 1: Untagged enum with Unknown fallback
//!
//! ```rust,ignore
//! #[derive(Deserialize)]
//! #[serde(untagged)]
//! pub enum EntityLegalForm {
//!     Known(KnownLegalForm),
//!     Unknown(String),  // Catches everything else
//! }
//!
//! #[derive(Deserialize)]
//! pub enum KnownLegalForm {
//!     #[serde(rename = "8888")]
//!     PublicLimitedCompany,
//!     // ...
//! }
//! ```
//!
//! ## Pattern 2: Store raw, map lazily (preferred for GLEIF)
//!
//! ```rust,ignore
//! pub struct GleifEntity {
//!     pub legal_form_code: String,  // Store verbatim from API
//! }
//!
//! impl GleifEntity {
//!     pub fn legal_form(&self) -> LegalForm {
//!         match self.legal_form_code.as_str() {
//!             "8888" => LegalForm::PublicLimitedCompany,
//!             "8889" => LegalForm::PrivateLimitedCompany,
//!             other => LegalForm::Unknown(other.to_string()),
//!         }
//!     }
//! }
//! ```
//!
//! ## The Rule
//!
//! > Capture the raw, map what you know, flag what you don't.
//!
//! This applies to all external integrations: GLEIF, BODS, screening providers,
//! market data feeds, etc.

pub mod client;
pub mod enrichment;
pub mod repository;
pub mod types;

pub use client::GleifClient;
pub use enrichment::GleifEnrichmentService;
pub use repository::GleifRepository;
pub use types::*;
