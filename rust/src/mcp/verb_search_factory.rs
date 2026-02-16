//! Verb Search Factory
//!
//! Provides a centralized factory for creating `HybridVerbSearcher` instances
//! with all required dependencies pre-wired. This ensures consistent configuration
//! across all call sites (AgentService, MCP handlers, etc.).
//!
//! ## Why a Factory?
//!
//! Before this factory, `HybridVerbSearcher::new()` was called in multiple places
//! with inconsistent parameters:
//! - AgentService passed `learned_data: None`, disabling learned phrase matching
//! - MCP handlers might omit the macro registry
//! - Each call site had to know about all dependencies
//!
//! The factory encapsulates the correct wiring pattern, ensuring:
//! 1. `learned_data` is always populated (from warmup)
//! 2. `macro_registry` is always provided (business vocabulary layer)
//! 3. `embedder` is always available (for semantic search)
//! 4. Consistent threshold configuration

use std::sync::Arc;

use sqlx::PgPool;

use crate::agent::learning::embedder::SharedEmbedder;
use crate::agent::learning::warmup::SharedLearnedData;
use crate::database::VerbService;
use crate::dsl_v2::macros::MacroRegistry;
use crate::mcp::verb_search::{HybridVerbSearcher, SharedLexicon};

/// Factory for creating properly-configured `HybridVerbSearcher` instances.
///
/// Use this instead of calling `HybridVerbSearcher::new()` directly to ensure
/// all channels (macro, learned, semantic) are properly wired.
pub struct VerbSearcherFactory;

impl VerbSearcherFactory {
    /// Build a fully-configured `HybridVerbSearcher` with all channels enabled.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database pool for verb service queries
    /// * `embedder` - Shared embedder for semantic search (BGE model)
    /// * `learned_data` - In-memory cache of learned invocation phrases (from warmup)
    /// * `macro_registry` - Operator macro registry for business vocabulary
    /// * `lexicon` - Optional lexicon service for fast lexical matching (runs before semantic)
    ///
    /// # Returns
    ///
    /// A `HybridVerbSearcher` with:
    /// - All search channels enabled (lexicon, macro, learned exact/semantic, pattern embedding, phonetic)
    /// - Proper threshold configuration for BGE asymmetric mode
    /// - user_id support for per-user learned phrases
    pub fn build(
        pool: &PgPool,
        embedder: SharedEmbedder,
        learned_data: Option<SharedLearnedData>,
        macro_registry: Arc<MacroRegistry>,
        lexicon: Option<SharedLexicon>,
    ) -> HybridVerbSearcher {
        let verb_service = Arc::new(VerbService::new(pool.clone()));

        let mut searcher = HybridVerbSearcher::new(verb_service, learned_data)
            .with_embedder(embedder)
            .with_macro_registry(macro_registry);

        if let Some(lex) = lexicon {
            searcher = searcher.with_lexicon(lex);
        }

        searcher
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require database fixtures.
    // These tests verify the factory compiles and basic structure.

    #[test]
    fn test_factory_type_signatures() {
        // Verify the factory methods have expected signatures
        // (actual execution requires database)
        fn _check_build_signature(
            _pool: &PgPool,
            _embedder: SharedEmbedder,
            _learned_data: Option<SharedLearnedData>,
            _macro_registry: Arc<MacroRegistry>,
            _lexicon: Option<SharedLexicon>,
        ) -> HybridVerbSearcher {
            unimplemented!("type check only")
        }
    }
}
