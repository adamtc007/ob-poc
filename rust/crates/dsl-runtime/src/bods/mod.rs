//! BODS (Beneficial Ownership Data Standard) module
//!
//! Provides types and services for working with beneficial ownership data
//! from Open Ownership registers (UK PSC, Denmark CVR, etc.)

pub mod repository;
pub mod types;
pub mod ubo_discovery;

pub use repository::BodsRepository;
pub use types::*;
pub use ubo_discovery::UboDiscoveryService;
