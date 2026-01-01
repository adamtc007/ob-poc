//! GLEIF (Global Legal Entity Identifier Foundation) integration
//!
//! This module provides:
//! - API types for GLEIF Level 1 (entity data) and Level 2 (relationships)
//! - Repository for persisting GLEIF data to our schema
//! - Client for fetching data from the GLEIF API
//! - Enrichment service for populating entities with GLEIF data

pub mod client;
pub mod enrichment;
pub mod repository;
pub mod types;

pub use client::GleifClient;
pub use enrichment::GleifEnrichmentService;
pub use repository::GleifRepository;
pub use types::*;
