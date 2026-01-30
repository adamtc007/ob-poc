//! Placeholder entity module.
//!
//! This module provides the infrastructure for creating and resolving placeholder
//! entities. Placeholders are stub entity records that are created when a macro
//! references an entity that doesn't exist yet (e.g., a depositary that hasn't
//! been selected).
//!
//! ## Lifecycle
//!
//! ```text
//! PENDING → RESOLVED → VERIFIED
//!     │         │          │
//!     │         │          └── Compliance/ops verified the resolution
//!     │         └────────────── Resolved to a real entity
//!     └──────────────────────── Created by macro expansion
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ob_poc::placeholder::{PlaceholderResolver, CreatePlaceholderRequest};
//!
//! let resolver = PlaceholderResolver::new(pool);
//!
//! // Create a placeholder for a depositary
//! let entity_id = resolver.create_placeholder(CreatePlaceholderRequest {
//!     kind: "depositary".to_string(),
//!     cbu_id: my_cbu_id,
//!     name_hint: Some("Depositary for Acme Fund".to_string()),
//!     description: None,
//! }).await?;
//!
//! // Later, resolve it to a real entity
//! let result = resolver.resolve(ResolvePlaceholderRequest {
//!     placeholder_entity_id: entity_id,
//!     resolved_entity_id: real_depositary_id,
//!     resolved_by: "user@example.com".to_string(),
//! }).await?;
//! ```

pub mod resolver;
pub mod types;

pub use resolver::PlaceholderResolver;
pub use types::{
    CreatePlaceholderRequest, PlaceholderEntity, PlaceholderKindCount, PlaceholderResolutionResult,
    PlaceholderStatus, PlaceholderSummary, PlaceholderWithDetails, ResolvePlaceholderRequest,
};
