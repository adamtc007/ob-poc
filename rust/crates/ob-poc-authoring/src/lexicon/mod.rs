//! Lexicon Service - In-memory vocabulary lookup for verb discovery
//!
//! This module provides a fast lexical search lane that runs BEFORE semantic
//! embedding computation in HybridVerbSearcher. It recognizes known vocabulary
//! (verb synonyms, entity types, domain keywords) and generates verb candidates
//! with explainable evidence.
//!
//! ## Architecture
//!
//! ```text
//! Build time:  YAML → LexiconCompiler → lexicon.snapshot.bin
//! Runtime:     load_binary() → Arc<LexiconSnapshot> → LexiconServiceImpl
//! ```
//!
//! ## Critical Rules
//!
//! 1. **Hot path = in-memory only** - NO DB queries, NO YAML parsing at query time
//! 2. **Scores clamped to [0, 1]** - Don't break existing ambiguity thresholds
//! 3. **Evidence maps to VerbEvidence** - No parallel evidence layer
//!
//! ## Usage
//!
//! ```rust,ignore
//! let snapshot = Arc::new(LexiconSnapshot::load_binary(path)?);
//! let service = LexiconServiceImpl::new(snapshot);
//!
//! let candidates = service.search_verbs("create fund", None, 5);
//! ```

mod compiler;
mod service;
mod snapshot;
mod types;

pub use compiler::LexiconCompiler;
pub use service::{LexiconService, LexiconServiceImpl};
pub use snapshot::LexiconSnapshot;
pub use types::*;
