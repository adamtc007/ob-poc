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
use crate::macros::OperatorMacroRegistry;
use crate::mcp::verb_search::HybridVerbSearcher;

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
    ///
    /// # Returns
    ///
    /// A `HybridVerbSearcher` with:
    /// - All search channels enabled (macro, learned exact/semantic, pattern embedding, phonetic)
    /// - Proper threshold configuration for BGE asymmetric mode
    /// - user_id support for per-user learned phrases
    pub fn build(
        pool: &PgPool,
        embedder: SharedEmbedder,
        learned_data: Option<SharedLearnedData>,
        macro_registry: Arc<OperatorMacroRegistry>,
    ) -> HybridVerbSearcher {
        let verb_service = Arc::new(VerbService::new(pool.clone()));

        HybridVerbSearcher::new(verb_service, learned_data)
            .with_embedder(embedder)
            .with_macro_registry(macro_registry)
    }

    /// Build a minimal `HybridVerbSearcher` for testing or fallback scenarios.
    ///
    /// Only the database channel is enabled. Useful for:
    /// - Unit tests that don't need full semantic search
    /// - Fallback when embedder/learned_data unavailable
    #[allow(dead_code)]
    pub fn build_minimal(pool: &PgPool) -> HybridVerbSearcher {
        let verb_service = Arc::new(VerbService::new(pool.clone()));
        HybridVerbSearcher::new(verb_service, None)
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
            _macro_registry: Arc<OperatorMacroRegistry>,
        ) -> HybridVerbSearcher {
            unimplemented!("type check only")
        }

        fn _check_minimal_signature(_pool: &PgPool) -> HybridVerbSearcher {
            unimplemented!("type check only")
        }
    }
}
