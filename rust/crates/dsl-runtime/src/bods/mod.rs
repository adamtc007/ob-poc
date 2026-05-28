//! BODS (Beneficial Ownership Data Standard) module
//!
//! Provides types and services for working with beneficial ownership data
//! from Open Ownership registers (UK PSC, Denmark CVR, etc.)

mod repository;
mod types;
mod ubo_discovery;

pub use repository::BodsRepository;
pub use types::{DiscoveredUbo, UboDiscoveryResult, UboType};
pub use ubo_discovery::UboDiscoveryService;
