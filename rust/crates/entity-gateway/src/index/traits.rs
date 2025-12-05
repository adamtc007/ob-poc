//! Core traits and types for the search index abstraction
//!
//! This module defines the `SearchIndex` trait that allows different
//! index implementations (Tantivy, SQLite FTS5, etc.) to be used
//! interchangeably.

use async_trait::async_trait;
use std::collections::HashMap;

/// A single search match result
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// The original input value that produced this match
    pub input: String,
    /// Human-readable display value (for UI)
    pub display: String,
    /// The resolved token/ID (UUID for DSL insertion)
    pub token: String,
    /// Relevance score (meaningful in fuzzy mode)
    pub score: f32,
}

/// Matching mode for search queries
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MatchMode {
    /// Fuzzy prefix matching with relevance ranking
    #[default]
    Fuzzy,
    /// Exact match only
    Exact,
}

/// A search query to execute against an index
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Values to search for
    pub values: Vec<String>,
    /// Which search key to use (e.g., "name", "email")
    pub search_key: String,
    /// Matching mode
    pub mode: MatchMode,
    /// Maximum results per input value
    pub limit: usize,
}

/// The main search index trait
///
/// Implementations must be Send + Sync for use in async contexts.
#[async_trait]
pub trait SearchIndex: Send + Sync {
    /// Execute a search query against this index
    ///
    /// Returns matches sorted by relevance (highest score first).
    async fn search(&self, query: &SearchQuery) -> Vec<SearchMatch>;

    /// Rebuild the index from source data
    ///
    /// This replaces all existing index data with the provided records.
    async fn refresh(&self, data: Vec<IndexRecord>) -> Result<(), IndexError>;

    /// Check if the index is ready to serve queries
    ///
    /// Returns false if the index hasn't been populated yet.
    fn is_ready(&self) -> bool;
}

/// A record to be indexed
#[derive(Debug, Clone)]
pub struct IndexRecord {
    /// The token/ID to return when matched
    pub token: String,
    /// Human-readable display value
    pub display: String,
    /// Map of search_key -> value for indexing
    pub search_values: HashMap<String, String>,
}

/// Errors that can occur during index operations
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Index build failed: {0}")]
    BuildFailed(String),
    #[error("Index not ready")]
    NotReady,
    #[error("Search failed: {0}")]
    SearchFailed(String),
}
