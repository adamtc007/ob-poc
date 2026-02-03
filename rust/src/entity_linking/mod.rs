//! Entity Linking Service
//!
//! Provides fast, in-memory entity resolution from natural language utterances.
//! Extracts entity mention spans and resolves them to canonical entity IDs
//! with disambiguation support via kind constraints and concept overlap.
//!
//! ## Architecture
//!
//! ```text
//! Build time:  DB tables → compiler → entity.snapshot.bin
//! Runtime:     load_binary() → Arc<EntitySnapshot> → EntityLinkingServiceImpl
//! ```
//!
//! ## Hot Path Performance
//!
//! All resolution is in-memory (no DB access):
//! - Alias index lookup: O(1)
//! - Token overlap matching: O(n * m) where n=query tokens, m=avg entities per token
//! - Target: p95 < 5ms
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ob_poc::entity_linking::{EntityLinkingServiceImpl, EntityLinkingService};
//!
//! let service = EntityLinkingServiceImpl::load_default()?;
//! let resolutions = service.resolve_mentions(
//!     "Set up Goldman Sachs for OTC trading",
//!     Some(&["company".to_string()]),
//!     None,
//!     5,
//! );
//!
//! for r in resolutions {
//!     println!("Found '{}' at {}..{}", r.mention_text, r.mention_span.0, r.mention_span.1);
//!     if let Some(id) = r.selected {
//!         println!("  Resolved to: {}", id);
//!     }
//! }
//! ```

pub mod compiler;
pub mod mention;
pub mod normalize;
pub mod resolver;
pub mod snapshot;

// Re-exports
pub use compiler::{compile_entity_snapshot, lint_entity_data, LintSeverity, LintWarning};
pub use mention::{MentionExtractor, MentionExtractorConfig, MentionSpan};
pub use normalize::{normalize_entity_text, tokenize};
pub use resolver::{
    EntityCandidate, EntityLinkingService, EntityLinkingServiceImpl, EntityResolution, Evidence,
};
pub use snapshot::{EntityId, EntityRow, EntitySnapshot, SnapshotStats, SNAPSHOT_VERSION};
