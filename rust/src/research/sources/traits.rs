//! SourceLoader trait and related types
//!
//! The core abstraction for pluggable research source loaders.

use super::normalized::{
    NormalizedControlHolder, NormalizedEntity, NormalizedOfficer, NormalizedRelationship,
};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Data types a source can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceDataType {
    /// Basic entity information (name, status, addresses)
    Entity,
    /// Control holders (>threshold% ownership/voting)
    ControlHolders,
    /// Officers and directors
    Officers,
    /// Parent chain (corporate hierarchy upward)
    ParentChain,
    /// Subsidiaries (corporate hierarchy downward)
    Subsidiaries,
    /// Regulatory filings
    Filings,
}

impl std::fmt::Display for SourceDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entity => write!(f, "entity"),
            Self::ControlHolders => write!(f, "control-holders"),
            Self::Officers => write!(f, "officers"),
            Self::ParentChain => write!(f, "parent-chain"),
            Self::Subsidiaries => write!(f, "subsidiaries"),
            Self::Filings => write!(f, "filings"),
        }
    }
}

// =============================================================================
// Argument Structs
// =============================================================================

/// Options for search operations
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Jurisdiction filter (ISO 3166-1 alpha-2)
    pub jurisdiction: Option<String>,
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Include inactive/dissolved entities
    pub include_inactive: bool,
}

impl SearchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_jurisdiction(mut self, jurisdiction: impl Into<String>) -> Self {
        self.jurisdiction = Some(jurisdiction.into());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn include_inactive(mut self) -> Self {
        self.include_inactive = true;
        self
    }
}

/// Options for fetching entity data
#[derive(Debug, Clone, Default)]
pub struct FetchOptions {
    /// Include raw API response in result (for audit)
    pub include_raw: bool,
    /// Decision ID for audit trail linkage
    pub decision_id: Option<uuid::Uuid>,
}

impl FetchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_raw(mut self) -> Self {
        self.include_raw = true;
        self
    }

    pub fn with_decision_id(mut self, id: uuid::Uuid) -> Self {
        self.decision_id = Some(id);
        self
    }
}

/// Options for fetching control holders
#[derive(Debug, Clone, Default)]
pub struct FetchControlHoldersOptions {
    /// Minimum ownership percentage threshold (default: source-specific, usually 5% or 25%)
    pub min_ownership_pct: Option<rust_decimal::Decimal>,
    /// Include ceased/historical control holders
    pub include_ceased: bool,
    /// Decision ID for audit trail linkage
    pub decision_id: Option<uuid::Uuid>,
}

impl FetchControlHoldersOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_min_ownership(mut self, pct: rust_decimal::Decimal) -> Self {
        self.min_ownership_pct = Some(pct);
        self
    }

    pub fn include_ceased(mut self) -> Self {
        self.include_ceased = true;
        self
    }

    pub fn with_decision_id(mut self, id: uuid::Uuid) -> Self {
        self.decision_id = Some(id);
        self
    }
}

/// Options for fetching officers
#[derive(Debug, Clone, Default)]
pub struct FetchOfficersOptions {
    /// Include resigned officers
    pub include_resigned: bool,
    /// Filter by role(s)
    pub roles: Option<Vec<String>>,
    /// Decision ID for audit trail linkage
    pub decision_id: Option<uuid::Uuid>,
}

impl FetchOfficersOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn include_resigned(mut self) -> Self {
        self.include_resigned = true;
        self
    }

    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = Some(roles);
        self
    }

    pub fn with_decision_id(mut self, id: uuid::Uuid) -> Self {
        self.decision_id = Some(id);
        self
    }
}

/// Options for fetching parent chain
#[derive(Debug, Clone, Default)]
pub struct FetchParentChainOptions {
    /// Maximum depth to traverse (None = unlimited)
    pub max_depth: Option<usize>,
    /// Decision ID for audit trail linkage
    pub decision_id: Option<uuid::Uuid>,
}

impl FetchParentChainOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn with_decision_id(mut self, id: uuid::Uuid) -> Self {
        self.decision_id = Some(id);
        self
    }
}

// =============================================================================
// Result Types
// =============================================================================

/// A candidate result from a search operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCandidate {
    /// Source-specific key (LEI, company number, CIK, etc.)
    pub key: String,
    /// Entity name as returned by source
    pub name: String,
    /// Jurisdiction code (ISO 3166-1 alpha-2)
    pub jurisdiction: Option<String>,
    /// Entity status (active, dissolved, etc.)
    pub status: Option<String>,
    /// Match score (0.0 - 1.0)
    pub score: f64,
    /// Source-specific metadata for disambiguation
    pub metadata: serde_json::Value,
}

// =============================================================================
// Trait Definition
// =============================================================================

/// Trait for pluggable research source loaders
///
/// Each source loader provides a consistent interface to external data sources
/// (GLEIF, Companies House, SEC EDGAR, etc.) that normalizes data to our entity model.
///
/// # Implementation Notes
///
/// - Implement rate limiting in `search` and `fetch_*` methods
/// - Return empty Vec rather than error for "no data available"
/// - Use `Unknown(String)` patterns for enums to handle unexpected API values
/// - Include raw response in `NormalizedEntity.raw_response` for audit when requested
#[async_trait]
pub trait SourceLoader: Send + Sync {
    /// Unique identifier for this source (e.g., "gleif", "companies-house")
    fn source_id(&self) -> &'static str;

    /// Human-readable name (e.g., "GLEIF - Global LEI Foundation")
    fn source_name(&self) -> &'static str;

    /// Jurisdictions this source covers
    ///
    /// Use ISO 3166-1 alpha-2 codes, or "*" for global sources.
    fn jurisdictions(&self) -> &[&'static str];

    /// Data types this source provides
    fn provides(&self) -> &[SourceDataType];

    /// Primary key type name (LEI, COMPANY_NUMBER, CIK, etc.)
    fn key_type(&self) -> &'static str;

    /// Validate a key format for this source
    ///
    /// Returns true if the key is valid for this source's key type.
    fn validate_key(&self, key: &str) -> bool;

    /// Search for entities by name (fuzzy)
    ///
    /// # Arguments
    ///
    /// * `query` - Search query (entity name, partial name, etc.)
    /// * `options` - Search options (jurisdiction, limit, etc.)
    ///
    /// # Returns
    ///
    /// Candidates sorted by match score (highest first).
    async fn search(
        &self,
        query: &str,
        options: Option<SearchOptions>,
    ) -> Result<Vec<SearchCandidate>>;

    /// Fetch entity by source-specific key
    ///
    /// # Arguments
    ///
    /// * `key` - Source-specific key (LEI, company number, CIK, etc.)
    /// * `options` - Fetch options
    async fn fetch_entity(
        &self,
        key: &str,
        options: Option<FetchOptions>,
    ) -> Result<NormalizedEntity>;

    /// Fetch control holders (>threshold% ownership/voting)
    ///
    /// Returns empty Vec if source doesn't provide this data.
    async fn fetch_control_holders(
        &self,
        key: &str,
        options: Option<FetchControlHoldersOptions>,
    ) -> Result<Vec<NormalizedControlHolder>>;

    /// Fetch officers/directors
    ///
    /// Returns empty Vec if source doesn't provide this data.
    async fn fetch_officers(
        &self,
        key: &str,
        options: Option<FetchOfficersOptions>,
    ) -> Result<Vec<NormalizedOfficer>>;

    /// Fetch parent chain (if available)
    ///
    /// Returns relationships from child → parent direction.
    /// Returns empty Vec if source doesn't provide this data.
    async fn fetch_parent_chain(
        &self,
        key: &str,
        options: Option<FetchParentChainOptions>,
    ) -> Result<Vec<NormalizedRelationship>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_data_type_display() {
        assert_eq!(SourceDataType::Entity.to_string(), "entity");
        assert_eq!(
            SourceDataType::ControlHolders.to_string(),
            "control-holders"
        );
        assert_eq!(SourceDataType::Officers.to_string(), "officers");
        assert_eq!(SourceDataType::ParentChain.to_string(), "parent-chain");
        assert_eq!(SourceDataType::Subsidiaries.to_string(), "subsidiaries");
        assert_eq!(SourceDataType::Filings.to_string(), "filings");
    }

    #[test]
    fn test_search_options_builder() {
        let opts = SearchOptions::new()
            .with_jurisdiction("GB")
            .with_limit(10)
            .include_inactive();

        assert_eq!(opts.jurisdiction, Some("GB".to_string()));
        assert_eq!(opts.limit, Some(10));
        assert!(opts.include_inactive);
    }

    #[test]
    fn test_fetch_options_builder() {
        let id = uuid::Uuid::new_v4();
        let opts = FetchOptions::new().with_raw().with_decision_id(id);

        assert!(opts.include_raw);
        assert_eq!(opts.decision_id, Some(id));
    }

    #[test]
    fn test_fetch_control_holders_options_builder() {
        let opts = FetchControlHoldersOptions::new()
            .with_min_ownership(rust_decimal::Decimal::from(25))
            .include_ceased();

        assert_eq!(
            opts.min_ownership_pct,
            Some(rust_decimal::Decimal::from(25))
        );
        assert!(opts.include_ceased);
    }
}
