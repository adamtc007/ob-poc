//! Unified Lookup Service
//!
//! Consolidates verb search and entity linking into a single analysis pass.
//! Implements **verb-first ordering**: verbs → expected_kinds → entities.
//!
//! ## Architecture
//!
//! ```text
//! User utterance: "Set up ISDA with Goldman Sachs"
//!         │
//!         ▼
//! LookupService.analyze()
//!         │
//!         ├─► 1. Verb search (lexicon + semantic)
//!         │       └─► "isda.create" (score: 0.88)
//!         │
//!         ├─► 2. Derive expected_kinds from verb schema
//!         │       └─► ["company", "counterparty"]
//!         │
//!         ├─► 3. Entity linking with kind constraints
//!         │       └─► "Goldman Sachs" → entity_id (boosted by kind match)
//!         │
//!         └─► LookupResult { verbs, entities, dominant_entity, expected_kinds }
//! ```
//!
//! ## Why Verb-First?
//!
//! 1. **Kind constraints** - Verb schema defines what entity types are valid
//! 2. **Disambiguation** - "Apple" as company vs person depends on verb context
//! 3. **Performance** - Skip entity search if verb doesn't take entity args

pub mod service;

pub use service::{LookupResult, LookupService};
